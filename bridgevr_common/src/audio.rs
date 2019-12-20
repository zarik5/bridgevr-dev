use crate::{ring_channel::*, sockets::*, *};
use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    *,
};
use log::*;
use safe_transmute::*;
use std::{cmp::min, collections::VecDeque, sync::*, thread::*, time::Duration, *};

const TRACE_CONTEXT: &str = "Audio IO";

const TIMEOUT: Duration = Duration::from_millis(500);

enum AudioMode {
    Input,
    Output,
    Loopback,
}

struct AudioSession {
    event_loop: Arc<EventLoop>,
    stream: StreamId,
    join_handle: Option<JoinHandle<()>>,
}

impl AudioSession {
    fn start(
        device_idx: Option<u64>,
        mode: AudioMode,
        mut buffer_callback: impl FnMut(StreamData) + Send + 'static,
    ) -> StrResult<AudioSession> {
        let host = cpal::default_host();
        let event_loop = Arc::new(host.event_loop());

        let mut devices_and_formats: Vec<_> = trace_err!(host.devices())?
            .filter_map(|dev| {
                match mode {
                    AudioMode::Input => dev.default_input_format(),
                    AudioMode::Output | AudioMode::Loopback => dev.default_output_format(),
                }
                .map(|format| (dev, format))
                .ok()
            })
            .collect();

        let devices_str =
            devices_and_formats
                .iter()
                .enumerate()
                .fold(String::new(), |s, (i, (dev, _))| {
                    s + &format!(
                        " {{ {}: {} }}",
                        i,
                        dev.name().unwrap_or_else(|_| "Unknown".into())
                    )
                });
        let io_str = match mode {
            AudioMode::Input => "input",
            AudioMode::Output => "output",
            AudioMode::Loopback => "loopback",
        };
        info!(
            "[{}] Audio {} devices:{}",
            TRACE_CONTEXT, io_str, devices_str
        );

        let (device, mut format) = if let Some(idx) = device_idx {
            let idx = idx as usize;
            if idx < devices_and_formats.len() {
                // the bound check prevents panic
                devices_and_formats.remove(idx)
            } else {
                trace_err!(Err("Index out of bound"))?
            }
        } else {
            match mode {
                AudioMode::Input => {
                    let dev = trace_none!(host.default_input_device())?;
                    let format = trace_err!(dev.default_input_format())?;
                    (dev, format)
                }
                AudioMode::Output | AudioMode::Loopback => {
                    let dev = trace_none!(host.default_output_device())?;
                    let format = trace_err!(dev.default_output_format())?;
                    (dev, format)
                }
            }
        };
        format.data_type = SampleFormat::F32;

        let stream = trace_err!(match mode {
            AudioMode::Input | AudioMode::Loopback => {
                event_loop.build_input_stream(&device, &format)
            }
            AudioMode::Output => event_loop.build_output_stream(&device, &format),
        })?;
        trace_err!(event_loop.play_stream(stream.clone()))?;

        let join_handle = Some(thread::spawn({
            let event_loop = event_loop.clone();
            move || {
                event_loop.run(move |_, maybe_data| match maybe_data {
                    Ok(io_data) => {
                        buffer_callback(io_data);
                    }
                    Err(e) => warn!("{}", e),
                });
            }
        }));

        Ok(AudioSession {
            event_loop,
            stream,
            join_handle,
        })
    }

    fn request_stop(&mut self) {
        // todo: check that this is non blocking
        self.event_loop.destroy_stream(self.stream.clone())
    }
}

impl Drop for AudioSession {
    fn drop(&mut self) {
        self.request_stop();
        self.join_handle.take().map(|h| h.join());
    }
}

pub struct AudioRecorder {
    session: AudioSession,
}

impl AudioRecorder {
    pub fn start_recording(
        device_idx: Option<u64>,
        loopback: bool,
        mut buffer_producer: Producer<SenderData>,
    ) -> StrResult<AudioRecorder> {
        let mode = if loopback {
            AudioMode::Loopback
        } else {
            AudioMode::Input
        };

        for _ in 0..3 {
            buffer_producer.add(SenderData {
                packet: vec![0; MAX_PACKET_SIZE_BYTES],
                data_offset: get_data_offset(&())?,
                data_size: 0,
            });
        }

        let mut buffer_index = 0;

        let session = trace_err!(AudioSession::start(device_idx, mode, move |io_data| {
            match io_data {
                StreamData::Input {
                    buffer: UnknownTypeInputBuffer::F32(samples),
                } => {
                    let res = buffer_producer.fill(TIMEOUT, |data| {
                        serialize_indexed_header_into(&mut data.packet, buffer_index, &())
                            .map_err(|e| error!("{}", e))
                            .ok();

                        let samples_bytes = guarded_transmute_to_bytes_pod_many(&samples[..]);
                        data.data_size = samples_bytes.len();

                        (&mut data.packet[data.data_offset..(data.data_offset + data.data_size)])
                            .copy_from_slice(samples_bytes);

                        Ok(())
                    });

                    if res.is_ok() {
                        buffer_index += 1;
                    }
                }
                _ => warn!("[Audio recorder] Invalid format"),
            }
        }))?;

        Ok(Self { session })
    }

    pub fn request_stop(&mut self) {
        self.session.request_stop()
    }
}

pub struct AudioPlayer {
    session: AudioSession,
}

impl AudioPlayer {
    fn copy_audio_buffer(input: &[u8], byte_count: usize, output: &mut VecDeque<f32>) {
        for chunk in input.chunks_exact(4).take(byte_count / 4) {
            output.push_back(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
    }

    pub fn start_playback(
        device_idx: Option<u64>,
        mut buffer_consumer: KeyedConsumer<ReceiverData<()>, u64>,
    ) -> StrResult<AudioPlayer> {
        let mut buffer_idx = 0;
        let mut sample_buffer = VecDeque::new();
        let session = trace_err!(AudioSession::start(
            device_idx,
            AudioMode::Output,
            move |io_data| {
                match io_data {
                    StreamData::Output {
                        buffer: UnknownTypeOutputBuffer::F32(mut samples),
                    } => {
                        let mut sample_idx = 0;
                        // todo: optimize code?
                        while sample_idx < samples.len() {
                            let samples_to_copy = min(samples.len(), sample_buffer.len());
                            samples[sample_idx..(sample_idx + samples_to_copy)]
                                .copy_from_slice(sample_buffer.as_slices().0);
                            sample_buffer.drain(0..samples_to_copy);
                            sample_idx += samples_to_copy;

                            if sample_idx < samples.len() {
                                let res = buffer_consumer
                                    .consume(&buffer_idx, Duration::from_secs(0), |data| {
                                        Self::copy_audio_buffer(
                                            &data.packet[..],
                                            data.packet_size,
                                            &mut sample_buffer,
                                        );
                                        Ok(())
                                    })
                                    .map_err(|e| debug!("{:?}", e));
                                if res.is_ok() {
                                    buffer_idx += 1;
                                } else {
                                    let res = buffer_consumer.consume_any(TIMEOUT, |idx, data| {
                                        Self::copy_audio_buffer(
                                            &data.packet[..],
                                            data.packet_size,
                                            &mut sample_buffer,
                                        );
                                        // todo: check buffer_idx is not a copy
                                        buffer_idx = idx + 1;
                                        Ok(())
                                    });
                                    if res.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => warn!("[Audio player] Invalid Format"),
                }
            }
        ))?;

        Ok(Self { session })
    }

    pub fn request_stop(&mut self) {
        self.session.request_stop()
    }
}
