mod graphics;
mod logging_backend;
mod openvr_backend;
mod statistics;
mod video_encoder;
mod utils;

use bridgevr_common::{
    audio_encoder::*, audio_recorder::AudioRecorder, packets::*, settings::*, sockets::*, *,
};
use graphics::Graphics;
use lazy_static::lazy_static;
use openvr_backend::*;
use std::{collections::HashMap, ffi::*, panic, sync::*};
use video_encoder::*;

const TRACE_CONTEXT: &str = "Server";

enum ClientBackend {
    Openvr(OpenvrClient), // todo: add OpenXR device plugin when it will be available
}

struct ClientBackendDesc {
    device_id: usize,
    settings: Arc<Settings>,
    target_eye_width: u32,
    target_eye_height: u32,
    handshake_packet: ClientHandshakePacket,
    graphics: Arc<Mutex<Graphics>>,
    video_encoder: Arc<Mutex<VideoEncoder>>,
}

struct Client {
    backend: ClientBackend,
    _audio_recorder: Option<AudioRecorder>,
}

struct Server {
    _settings: Arc<Settings>,
    _clients: Arc<Mutex<HashMap<usize, Client>>>,
    _device_id_counter: Arc<Mutex<usize>>,
    handshake_socket: Mutex<HandshakeSocket>,
}

fn present(texture_handle: u64, graphics: Arc<Mutex<Graphics>>) {
    // graphics
    //     .lock()
    //     .unwrap()
    //     .select_input_texture(texture_handle)
}

fn wait_for_present(graphics: Arc<Mutex<Graphics>>, video_encoder: Arc<Mutex<VideoEncoder>>) {
    graphics.lock().unwrap().render();
    video_encoder.lock().unwrap().encode(false);
}

fn handle_packet(
    clients: Arc<Mutex<HashMap<usize, Client>>>,
    device_id: usize,
    client_packet: Option<&ClientMessage>,
) {
    match client_packet {
        Some(ClientMessage::HmdAndControllers {
            hmd_pose,
            input_devices_data,
            additional_vsync_offset_ns,
        }) => {
            // `client` is still not available in the scope, so i'm obtaining it with
            // `'device_id`. This is because of RAII prohibiting circular referencing
            if let Some(client) = (*clients.lock().unwrap()).get_mut(&device_id) {
                match &mut client.backend {
                    ClientBackend::Openvr(client) => {
                        let controller_poses = vec![];

                        // input_devices_data.iter().map(|d| {

                        // }).collect();

                        // todo: define openvr input interface
                        client.update_input(hmd_pose, &controller_poses)
                    }
                }
            }
        }
        Some(ClientMessage::Shutdown) => {
            clients.lock().unwrap().remove(&device_id);

            // shutdown openvr
        }
        None => {
            clients.lock().unwrap().remove(&device_id);
        }
        _ => unimplemented!(), // todo: microphone
    }
}

// The following complex system of callbacks is needed to satisfy Rust's RAII
// requirement without workarounds.
// Some objects don't drop because they live inside the context of a callback
fn create_server(
    create_client_callback: fn(ClientBackendDesc) -> ClientBackend,
    shutdown_backend_callback: fn(),
) -> StrResult<Server> {
    let settings = Arc::new(trace_err!(
        load_settings(env!("SETTINGS_PATH")),
        "Create server"
    )?);
    let clients: Arc<Mutex<HashMap<usize, Client>>> = <_>::default();
    let device_id_counter = Arc::new(Mutex::new(0));

    let handshake_socket = Mutex::new(HandshakeSocket::start_listening(
        settings.connections.clone(),
        {
            let settings = settings.clone();
            let clients = clients.clone();
            let device_id_counter = device_id_counter.clone();
            move |handshake_message| -> StrResult<_> {
                let (target_eye_width, target_eye_height) = match settings.video.frame_size {
                    FrameSize::Scale(scale) => {
                        let width = (handshake_message.native_eye_width as f32 * scale) as _;
                        let height = (handshake_message.native_eye_height as f32 * scale) as _;
                        (width, height)
                    }
                    FrameSize::Absolute { width, height } => (width, height),
                };

                let device_id = {
                    let mut device_id_counter = device_id_counter.lock().unwrap();
                    let device_id = *device_id_counter;
                    *device_id_counter += 1;
                    device_id
                };

                let frame_counter = Arc::new(Mutex::new(0));

                let graphics = Arc::new(Mutex::new(trace_err!(Graphics::new(
                    target_eye_width,
                    target_eye_height,
                    None,
                ))?));

                match settings.video.slices {
                    Slices::Count(count) => {}
                    Slices::Size { max_pixels } => {}
                }

                let audio_recorder = match settings.audio.clone() {
                    Switch::Enabled(Audio {
                        encoder: encoder_desc,
                        ..
                    }) => {
                        let audio_encoder = Arc::new(Mutex::new(AudioEncoder::new(encoder_desc)));
                        Some(AudioRecorder::start(|sample| {
                            audio_encoder.lock().unwrap().encode(sample);
                        }))
                    }
                    Switch::Disabled => None,
                };

                let handshake_packet = ServerHandshakePacket {
                    settings: (*settings).clone(),
                    target_eye_width,
                    target_eye_height,
                };

                let connection_callback = {
                    let settings = settings.clone();
                    let clients = clients.clone();

                    move |connection_socket: ConnectionManager<ServerMessage, usize>| -> StrResult<()> {

                        // let video_encoder = Arc::new(Mutex::new(VideoEncoder::new(
                        //     settings.video.encoder.clone(),
                        //     |video_data| {
                        //         let packet = ServerMessage::Video {
                        //             frame_idx: *frame_counter.lock().unwrap(),
                        //             sub_frame_idx: 0,
                        //             video_nal: Vec::from(video_data),
                        //         };

                        //         connection_socket.send_data(&packet);
                        //     },
                        // )));

                        let backend = create_client_callback(ClientBackendDesc {
                            device_id,
                            settings,
                            target_eye_width,
                            target_eye_height,
                            handshake_packet,
                            graphics,
                            video_encoder,
                        });

                        let client = Client {
                            backend,
                            _audio_recorder: audio_recorder,
                        };

                        clients.lock().unwrap().insert(device_id, client);
                        // })
                        // .map_err(|_| shutdown_backend_callback())
                        // .ok();

                        Ok(())
                    }
                };

                let message_received_callback = {
                    let clients = clients.clone();

                    move |client_packet: ClientMessage| -> StrResult<()> {
                        // handle_packet();
                        Ok(())
                    }
                };

                Ok(ConnectionDesc {
                    handshake_packet,
                    connection_callback,
                    message_received_callback,
                })
            }
        },
    ));

    Ok(Server {
        _settings: settings,
        _clients: clients,
        _device_id_counter: device_id_counter,
        handshake_socket,
    })
}

////////////////////////////////////////////// OpenVR /////////////////////////////////////////////

use openvr_driver::*;

openvr_server_entry_point!(
    (|| -> Result<&ServerTrackedDeviceProvider<ServerContext>, ()> {
        logging_backend::init_logging();

        lazy_static! {
            static ref MAYBE_SERVER_REF: StrResult<Server> = create_server(
                |client_desc| {
                    ClientBackend::Openvr(OpenvrClient::new(
                        client_desc.device_id,
                        client_desc.settings,
                        client_desc.target_eye_width,
                        client_desc.target_eye_height,
                        client_desc.handshake_packet,
                        {
                            let graphics = client_desc.graphics.clone();
                            move |texture_handle| present(texture_handle, graphics.clone())
                        },
                        {
                            let graphics = client_desc.graphics.clone();
                            let video_encoder = client_desc.video_encoder.clone();
                            move || wait_for_present(graphics.clone(), video_encoder.clone())
                        },
                    ))
                },
                || {
                    OPENVR_SERVER.lock().unwrap().shutdown();
                }
            );
            static ref OPENVR_SERVER: Mutex<OpenvrServer> = Mutex::new(OpenvrServer::new(|| {
                if let Ok(server) = *MAYBE_SERVER_REF {
                    server.handshake_socket.lock().unwrap().shutdown()
                }
            }));
        }

        display_err!(MAYBE_SERVER_REF)?;

        Ok(OPENVR_SERVER.lock().unwrap().to_native())
    })()
);
