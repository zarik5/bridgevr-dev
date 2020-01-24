use crate::{
    data::*,
    event_timing::*,
    sockets::*,
    thread_loop::{self, *},
    *,
};
use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    *,
};
use log::*;
use safe_transmute::*;
use std::{cmp::min, sync::mpsc::*, sync::*, thread::*, time::Duration, time::*, *};

const TRACE_CONTEXT: &str = "Audio";

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
                return trace_str!("Index out of bound");
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

        let join_handle = Some(trace_err!(thread::Builder::new()
            .name("Audio thread".into())
            .spawn({
                let event_loop = event_loop.clone();
                move || {
                    event_loop.run(move |_, maybe_data| match maybe_data {
                        Ok(io_data) => {
                            buffer_callback(io_data);
                        }
                        Err(e) => warn!("{}", e),
                    });
                }
            }))?);

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
        mut packet_enqueuer: PacketEnqueuer,
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
                    let audio_packet = AudioPacket {
                        samples: transmute_to_bytes(&samples[..]),
                    };

                    packet_enqueuer
                        .enqueue(&audio_packet)
                        .map_err(|e| debug!("{}", e))
                        .ok();
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

// In the case of buffer underrun, the audio player just wait for new samples.
// In case the difference between the target sample index and the dequeued buffer sample index is
// higher than a threshold, jump straight to the received buffer sample index.
// If not the cases above, if sample dequeue returns immediately then enter resync mode where every
// n samples a sample is dropped.

pub struct AudioPlayer {
    session: AudioSession,
    packet_timestamp_thread: ThreadLoop,
}

impl AudioPlayer {
    pub fn start_playback(
        device_idx: Option<u64>,
        latency_desc: LatencyDesc,
        mut packet_dequeuer: PacketDequeuer,
    ) -> StrResult<AudioPlayer> {
        let (timestamp_packet_sender, timestamp_packet_receiver) = channel();

        let packet_timestamp_thread =
            thread_loop::spawn("Audio player packet forward loop", move || {
                let maybe_packet = packet_dequeuer
                    .dequeue(TIMEOUT)
                    .map_err(|e| debug!("{}", e));

                if let Ok(packet) = maybe_packet {
                    let maybe_audio_packet =
                        packet.get::<AudioPacket>().map_err(|e| debug!("{}", e));
                    if let Ok(audio_packet) = maybe_audio_packet {
                        // Ignore the packet if transmute fails. The chance of a packet having the
                        // length corrupted but resulting valid by bincode is non existent
                        if let Ok(samples) =
                            transmute_many::<f32, PermissiveGuard>(audio_packet.samples)
                        {
                            timestamp_packet_sender
                                .send((Instant::now(), samples.to_vec()))
                                .map_err(|e| debug!("{}", e))
                                .ok();
                        }
                    }
                }
            })?;

        let sample_rate_hz = 44100_f32; // todo query
        let default_buffer_size = 1024_f32; // todo update
        let notifs_per_sec = sample_rate_hz / default_buffer_size;

        let callback_max_duration = Duration::from_secs_f32(1_f32 / notifs_per_sec);

        let mut event_timing = EventTiming::new(latency_desc, notifs_per_sec);

        // Contains unused samples from the previous packet
        let mut sample_buffer = vec![];

        let session = trace_err!(AudioSession::start(
            device_idx,
            AudioMode::Output,
            move |io_data| {
                let callback_begin_time = Instant::now();
                let callback_underrun_deadline = callback_begin_time + callback_max_duration;

                match io_data {
                    StreamData::Output {
                        buffer: UnknownTypeOutputBuffer::F32(mut samples),
                    } => {
                        let mut samples = &mut samples[..];
                        let max_dequeue_count = min(samples.len(), sample_buffer.len());

                        samples[0..max_dequeue_count].copy_from_slice(
                            &sample_buffer
                                .drain(0..max_dequeue_count)
                                .collect::<Vec<_>>(),
                        );
                        samples = &mut samples[max_dequeue_count..];

                        while !samples.is_empty() {
                            let estimated_underrun_timeout =
                                if let Some(estimated_underrun_timeout) =
                                    callback_max_duration.checked_sub(callback_begin_time.elapsed())
                                {
                                    estimated_underrun_timeout
                                } else {
                                    break;
                                };

                            let (arrival_timestamp, received_samples) = if let Ok(pair) =
                                timestamp_packet_receiver
                                    .recv_timeout(estimated_underrun_timeout)
                                    .map_err(|e| debug!("{}", e))
                            {
                                pair
                            } else {
                                break;
                            };

                            event_timing
                                .notify_latency(callback_underrun_deadline - arrival_timestamp);
                            if let Some(target_latency_deviation) = event_timing
                                .average_latency()
                                .checked_sub(event_timing.target_latency())
                            {
                                if target_latency_deviation > callback_max_duration {
                                    // the packet queue length was > 1 for a while, so drop one
                                    // packet.

                                    // since EventTiming has "momentum", add a fake packet notify to
                                    // normalize its state and to avoid dropping packets until
                                    // underrun.
                                    event_timing.notify_latency(event_timing.target_latency());

                                    continue;
                                }
                            }

                            let max_copy_count = min(samples.len(), received_samples.len());
                            samples[0..max_copy_count]
                                .copy_from_slice(&received_samples[0..max_copy_count]);
                            samples = &mut samples[max_dequeue_count..];

                            if max_copy_count < received_samples.len() {
                                // fill sample_buffer with remaining samples
                                sample_buffer = received_samples[max_copy_count..].to_vec();
                                break;
                            }
                        }

                        if callback_begin_time.elapsed() > callback_max_duration {
                            debug!("Audio player underrun!");

                            // fake packet notify to account for no packets found
                            event_timing.notify_latency(Duration::new(0, 0))
                        }
                    }
                    _ => warn!("[Audio player] Invalid Format"),
                }
            }
        ))?;

        Ok(Self {
            session,
            packet_timestamp_thread,
        })
    }

    pub fn request_stop(&mut self) {
        self.session.request_stop();
        self.packet_timestamp_thread.request_stop()
    }
}
