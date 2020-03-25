mod compositor;
mod logging_backend;
mod motion_model_3dof;
mod openvr;
mod shutdown_signal;
mod statistics;
mod video_encoder;

use bridgevr_common::{audio::*, constants::*, data::*, graphics::*, sockets::*, *};
use compositor::*;
use lazy_static::lazy_static;
use log::*;
use openvr::*;
use parking_lot::Mutex;
use shutdown_signal::ShutdownSignal;
use statistics::*;
use std::{
    ffi::*,
    os::raw::*,
    ptr::null_mut,
    sync::{mpsc::*, *},
    thread,
    time::*,
};
use video_encoder::*;

const TRACE_CONTEXT: &str = "Driver main";

const TIMEOUT: Duration = Duration::from_secs(1);

const STATISTICS_MAX_INTERVAL: Duration = Duration::from_secs(1);

fn get_settings() -> StrResult<Settings> {
    load_settings(env!("SETTINGS_PATH"))
}

fn begin_server_loop(
    graphics: Arc<GraphicsContext>,
    vr_server: Arc<Mutex<VrServer>>,
    shutdown_signal_sender: Sender<ShutdownSignal>,
    shutdown_signal_receiver: Receiver<ShutdownSignal>,
    session_desc_loader: Arc<Mutex<SessionDescLoader>>,
) -> StrResult {
    let timeout = get_settings()
        .map(|s| Duration::from_secs(s.openvr.server_idle_timeout_s))
        .unwrap_or(TIMEOUT);
    let mut deadline = Instant::now() + timeout;

    let try_connect = {
        let vr_server = vr_server.clone();
        move |shutdown_signal_receiver: &Receiver<ShutdownSignal>| -> StrResult<ShutdownSignal> {
            let settings = if let Ok(settings) = get_settings() {
                settings
            } else {
                thread::sleep(TIMEOUT);
                get_settings()?
            };

            let (found_client_ip, client_handshake_packet) =
                search_client(settings.connection.client_ip.clone(), TIMEOUT)?;

            if client_handshake_packet.version < BVR_MIN_VERSION_CLIENT {
                return trace_str!(
                    "Espected client of version {} or greater, found {}.",
                    BVR_MIN_VERSION_CLIENT,
                    client_handshake_packet.version
                );
            }

            session_desc_loader
                .lock()
                .get_mut()
                .last_client_handshake_packet = Some(client_handshake_packet.clone());
            session_desc_loader
                .lock()
                .save()
                .map_err(|e| warn!("{}", e))
                .ok();

            let target_eye_resolution = match &settings.video.frame_size {
                FrameSize::Scale(scale) => {
                    let (native_eye_width, native_eye_height) =
                        client_handshake_packet.native_eye_resolution;
                    let width = (native_eye_width as f32 * *scale) as _;
                    let height = (native_eye_height as f32 * *scale) as _;
                    (width, height)
                }
                FrameSize::Absolute(width, height) => (*width, *height),
            };

            let server_handshake_packet = ServerHandshakePacket {
                config: ServerConfig {
                    version: BVR_VERSION_SERVER,
                    target_eye_resolution,
                },
                settings: settings.clone(),
            };

            let mut connection_manager = ConnectionManager::connect_to_client(
                found_client_ip,
                settings.connection.config.clone(),
                server_handshake_packet,
                {
                    let shutdown_signal_sender = shutdown_signal_sender.clone();

                    // timeout callback
                    move || {
                        shutdown_signal_sender
                            .send(ShutdownSignal::ClientDisconnected)
                            .ok();
                    }
                },
            )?;

            let (present_sender, present_receiver) = channel();
            let (present_done_notif_sender, present_done_notif_receiver) = channel();

            let mut slice_senders = vec![];
            let mut slice_encoded_notif_receivers = vec![];
            let mut slice_interop_encoders = vec![];
            for _ in 0..settings.video.frame_slice_count {
                let (slice_sender, slice_receiver) = channel();
                let (slice_encoded_notif_sender, slice_encoded_notif_receiver) = channel();
                slice_senders.push(slice_sender);
                slice_encoded_notif_receivers.push(slice_encoded_notif_receiver);
                slice_interop_encoders.push((slice_receiver, slice_encoded_notif_sender));
            }

            let mut compositor = Compositor::new(
                graphics.clone(),
                CompositorDesc {
                    target_eye_resolution,
                    filter_type: settings.video.composition_filtering,
                    ffr_desc: settings.video.foveated_rendering.clone().into_option(),
                },
                present_receiver,
                present_done_notif_sender,
                slice_senders,
                slice_encoded_notif_receivers,
            )?;

            let video_encoder_resolution = compositor.encoder_resolution();

            let mut video_encoders = vec![];
            for (idx, (slice_receiver, slice_encoded_notif_sender)) in
                slice_interop_encoders.into_iter().enumerate()
            {
                let send_mode = if settings.video.reliable {
                    SendMode::ReliableOrdered
                } else {
                    SendMode::UnreliableSequential
                };
                let packet_enqueuer = connection_manager
                    .register_enqueuer(StreamType::VideoSlice(idx as _), send_mode);

                video_encoders.push(VideoEncoder::new(
                    &format!("Video encoder loop {}", idx),
                    settings.video.encoder.clone(),
                    video_encoder_resolution,
                    client_handshake_packet.fps,
                    slice_receiver,
                    slice_encoded_notif_sender,
                    packet_enqueuer,
                )?);
            }

            let mut maybe_game_audio_recorder = match &settings.game_audio {
                Switch::Enabled(desc) => {
                    let send_mode = if desc.reliable {
                        SendMode::ReliableOrdered
                    } else {
                        SendMode::UnreliableSequential
                    };
                    let packet_enqueuer =
                        connection_manager.register_enqueuer(StreamType::GameAudio, send_mode);

                    Some(AudioRecorder::start_recording(
                        desc.input_device_index,
                        true,
                        packet_enqueuer,
                    )?)
                }
                Switch::Disabled => None,
            };

            let mut maybe_microphone_player = match &settings.microphone {
                Switch::Enabled(desc) => {
                    let packet_dequeuer =
                        connection_manager.register_dequeuer(StreamType::Microphone);

                    Some(AudioPlayer::start_playback(
                        desc.output_device_index,
                        desc.buffering_latency.clone(),
                        packet_dequeuer,
                    )?)
                }
                Switch::Disabled => None,
            };

            let haptic_enqueuer = connection_manager
                .register_enqueuer(StreamType::Other, SendMode::UnreliableUnordered);

            vr_server.lock().initialize_for_client_or_request_restart(
                &settings,
                session_desc_loader.lock().get_mut(),
                present_sender,
                present_done_notif_receiver,
                haptic_enqueuer,
            )?;

            let mut other_packet_dequeuer = connection_manager.register_dequeuer(StreamType::Other);
            let shutdown_signal = loop {
                if let Ok(packet) = other_packet_dequeuer.dequeue(STATISTICS_MAX_INTERVAL) {
                    match packet.get::<OtherClientPacket>() {
                        Ok(OtherClientPacket::MotionAndTiming {
                            device_motions,
                            virtual_vsync_offset_ns,
                        }) => {
                            let mut vr_server = vr_server.lock();
                            for device_motion in device_motions {
                                let sample_6dof = match device_motion.sample {
                                    MotionSampleDesc::Dof6(sample) => sample,
                                    MotionSampleDesc::Dof3(_) => {
                                        // todo: use 3dof to 6dof model
                                        todo!()
                                    }
                                };

                                vr_server.process_motion(
                                    device_motion.device_type,
                                    sample_6dof,
                                    device_motion.timestamp_ns,
                                );
                            }
                            vr_server.update_virtual_vsync(virtual_vsync_offset_ns);
                        }
                        Ok(OtherClientPacket::InputDeviceData { data, timestamp_ns }) => {
                            vr_server.lock().process_input(data, timestamp_ns)
                        }
                        Ok(OtherClientPacket::Statistics(_)) => {
                            log_statistics(); // todo
                        }
                        Ok(OtherClientPacket::Disconnected) => {
                            break ShutdownSignal::ClientDisconnected
                        }
                        Err(e) => debug!("{}", e),
                    }
                }

                match shutdown_signal_receiver.try_recv() {
                    Ok(signal) => break signal,
                    Err(TryRecvError::Disconnected) => break ShutdownSignal::BackendShutdown,
                    Err(TryRecvError::Empty) => continue,
                }
            };

            connection_manager
                .register_enqueuer(StreamType::Other, SendMode::ReliableUnordered)
                .enqueue(&())
                .ok();

            connection_manager.request_stop();
            compositor.request_stop();

            for video_encoder in &mut video_encoders {
                video_encoder.request_stop();
            }

            if let Some(recorder) = &mut maybe_game_audio_recorder {
                recorder.request_stop();
            }

            if let Some(player) = &mut maybe_microphone_player {
                player.request_stop();
            }

            Ok(shutdown_signal)
        }
    };

    trace_err!(thread::Builder::new()
        .name("Connection/statistics loop".into())
        .spawn(move || while Instant::now() < deadline {
            match show_err!(try_connect(&shutdown_signal_receiver)) {
                Ok(ShutdownSignal::ClientDisconnected) => deadline = Instant::now() + timeout,
                Ok(ShutdownSignal::BackendShutdown) => break,
                Err(()) => {
                    if let Ok(ShutdownSignal::BackendShutdown) | Err(TryRecvError::Disconnected) =
                        shutdown_signal_receiver.try_recv()
                    {
                        break;
                    }
                }
            }
            vr_server.lock().deinitialize_for_client();
        })
        .map(|_| ()))
}

struct EmptySystem {
    graphics: Arc<GraphicsContext>,
    vr_server: Arc<Mutex<VrServer>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
    shutdown_signal_receiver_temp: Arc<Mutex<Option<Receiver<ShutdownSignal>>>>,
    session_desc_loader: Arc<Mutex<SessionDescLoader>>,
}

fn create_empty_system() -> StrResult<EmptySystem> {
    let maybe_settings = get_settings()
        .map_err(|_| error!("Cannot read settings. BridgeVR server will be in an invalid state."))
        .ok();

    let session_desc_loader = Arc::new(Mutex::new(SessionDescLoader::load(env!("SESSION_PATH"))));

    let graphics = Arc::new(GraphicsContext::new(None)?);

    let (shutdown_signal_sender, shutdown_signal_receiver) = mpsc::channel();

    let vr_server = Arc::new(Mutex::new(VrServer::new(
        graphics.clone(),
        maybe_settings.as_ref(),
        &session_desc_loader.lock().get_mut(),
        shutdown_signal_sender.clone(),
    )));

    Ok(EmptySystem {
        graphics,
        vr_server,
        shutdown_signal_sender: Arc::new(Mutex::new(shutdown_signal_sender)),
        shutdown_signal_receiver_temp: Arc::new(Mutex::new(Some(shutdown_signal_receiver))),
        session_desc_loader,
    })
}

// OpenVR entry point
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn HmdDriverFactory(
    interface_name: *const c_char,
    return_code_ptr: *mut c_int,
) -> *mut c_void {
    use openvr_driver_sys as vr;
    logging_backend::init_logging();

    // lazy_static! {
    //     static ref MAYBE_EMPTY_SYSTEM: StrResult<EmptySystem> = create_empty_system();
    // }

    // let try_create_server = || -> StrResult<_> {
    //     let sys = (*MAYBE_EMPTY_SYSTEM).as_ref()?;
    //     begin_server_loop(
    //         sys.graphics.clone(),
    //         sys.vr_server.clone(),
    //         sys.shutdown_signal_sender.lock().clone(),
    //         // this unwrap is safe because `shutdown_signal_receiver_temp` has just been set
    //         sys.shutdown_signal_receiver_temp.lock().take().unwrap(),
    //         sys.session_desc_loader.clone(),
    //     )?;

    //     Ok(sys.vr_server.lock().server_ptr())
    // };

    // match try_create_server() {
    //     Ok(mut server_ptr) => {
    //         if CStr::from_ptr(interface_name)
    //             == CStr::from_bytes_with_nul_unchecked(vr::IServerTrackedDeviceProvider_Version)
    //         {
    //             server_ptr = null_mut();
    //         }

    //         if server_ptr.is_null() && !return_code_ptr.is_null() {
    //             *return_code_ptr = vr::VRInitError_Init_InterfaceNotFound as _;
    //         }

    //         server_ptr as _
    //     }
    //     Err(e) => {
    //         show_err_str!("{}", e);
    //         null_mut()
    //     }
    // }

    show_err_str!("Hello from steamvr");
    null_mut()
}
