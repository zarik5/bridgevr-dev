use crate::{ring_buffer::*, *};
use byteorder::*;
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use cpal::*;
use log::error;
use log::info;
use std::collections::VecDeque;
use std::time::Duration;
use std::{sync::*, *};

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
                        dev.name().unwrap_or(String::from("Unknown"))
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
            AudioMode::Input | AudioMode::Loopback =>
                event_loop.build_input_stream(&device, &format),
            AudioMode::Output => event_loop.build_output_stream(&device, &format),
        })?;
        trace_err!(event_loop.play_stream(stream.clone()))?;

        thread::spawn({
            let event_loop = event_loop.clone();
            move || {
                event_loop.run(move |_, maybe_data| match maybe_data {
                    Ok(io_data) => {
                        buffer_callback(io_data);
                    }
                    Err(e) => error!("{}", e),
                });
            }
        });

        Ok(AudioSession { event_loop, stream })
    }

    fn stop(&self) {
        self.event_loop.destroy_stream(self.stream.clone())
    }
}

impl Drop for AudioSession {
    fn drop(&mut self) {
        self.stop()
    }
}

pub struct AudioRecorder {
    session: AudioSession,
}

impl AudioRecorder {
    pub fn start_recording(
        device_idx: Option<u64>,
        loopback: bool,
        mut samples_received_callback: impl FnMut(&[f32]) + Send + 'static,
    ) -> StrResult<AudioRecorder> {
        let mode = if loopback {
            AudioMode::Loopback
        } else {
            AudioMode::Input
        };

        let session = trace_err!(AudioSession::start(device_idx, mode, move |io_data| {
            match io_data {
                StreamData::Input {
                    buffer: UnknownTypeInputBuffer::F32(samples),
                } => {
                    samples_received_callback(&samples);
                }
                _ => error!("[Audio recorder] Invalid format"),
            }
        }))?;

        Ok(Self { session })
    }

    pub fn stop(&self) {
        self.session.stop();
    }
}

pub struct AudioPlayer {
    session: AudioSession,
}

impl AudioPlayer {
    fn copy_audio_buffer(input: &Box<[u8]>, byte_count: u64, output: &mut VecDeque<f32>) {
        // todo: use from_le_bytes() when is stabilized #60446
        for chunk in input.chunks_exact(4).take(byte_count as usize / 4) {
            output.push_back(LittleEndian::read_f32(chunk));
        }
    }

    pub fn start_playback(
        device_idx: Option<u64>,
        mut buffer_consumer: KeyedConsumer<(Box<[u8]>, u64), u64>,
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
                            while sample_idx < samples.len() {
                                if let Some(sample) = sample_buffer.pop_front() {
                                    samples[sample_idx] = sample;
                                    sample_idx += 1;
                                } else {
                                    break;
                                }
                            }
                            if sample_idx < samples.len() {
                                let res = buffer_consumer.consume(
                                    &buffer_idx,
                                    Duration::from_secs(0),
                                    |(packet_buffer, byte_count)| {
                                        Self::copy_audio_buffer(
                                            packet_buffer,
                                            *byte_count,
                                            &mut sample_buffer,
                                        );
                                        Ok(())
                                    },
                                );
                                if res.is_ok() {
                                    buffer_idx += 1;
                                } else {
                                    let res = buffer_consumer.consume_any(
                                        TIMEOUT,
                                        |idx, (packet_buffer, byte_count)| {
                                            Self::copy_audio_buffer(
                                                packet_buffer,
                                                *byte_count,
                                                &mut sample_buffer,
                                            );
                                            // todo: check buffer_idx is not a copy
                                            buffer_idx = idx + 1;
                                            Ok(())
                                        },
                                    );
                                    if res.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => error!("[Audio player] Invalid Format"),
                }
            }
        ))?;

        Ok(Self { session })
    }

    pub fn stop(&self) {
        self.session.stop();
    }
}
