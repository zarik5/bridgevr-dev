mod compositor;
mod logging_backend;
mod vrclient;

#[cfg(target_os = "android")]
mod android_audio;

use bridgevr_common::{data::*, graphics::*, sockets::*, *};
use compositor::*;
use log::debug;
use parking_lot::*;
use std::{
    sync::{atomic::*, Arc},
    thread,
    time::*,
};
use vrclient::*;

const TRACE_CONTEXT: &str = "App main";

const TIMEOUT: Duration = Duration::from_millis(500);

fn begin_client_loop(
    compositor: Arc<Mutex<Compositor>>,
    vr_client: Arc<Mutex<VrClient>>,
    connected_to_server: Arc<AtomicBool>,
) -> StrResult {
    let try_connect = {
        let vr_client = vr_client.clone();
        let compositor = compositor.clone();
        move || -> StrResult {
            // let client_handshake_packet = ClientHandshakePacket {
            //     bridgevr_name: constants::BVR_NAME.into(),
            //     version: constants::BVR_VERSION_CLIENT,
            //     native_eye_resolution: vr_client.lock().native_eye_resolution(),
            //     fov: vr_client.lock().fov(),
            //     fps: vr_client.lock().fps(),
            // };
            // let (connection_manager, server_handshake_packet) =
            //     ConnectionManager::connect_to_server(client_handshake_packet, |server_message| {
            //         match server_message {
            //             ServerMessage::Haptic(data) => {
            //                 //todo
            //             }
            //             ServerMessage::Shutdown => {
            //                 //todo
            //             }
            //         }
            //     })?;
            // let connection_manager = Arc::new(Mutex::new(connection_manager));
            // let settings = server_handshake_packet.settings;

            // let sender_data_port = settings.connection.starting_data_port;
            // let mut next_receiver_data_port = settings.connection.starting_data_port;

            // // connection_manager.send_message_udp(packet: &SM);

            // let maybe_game_audio_player = match settings.game_audio {
            //     Switch::Enabled(desc) => {
            //         let (producer, consumer) = keyed_channel_split(TIMEOUT);
            //         connection_manager.lock().begin_receive_indexed_buffers(
            //             "Game audio receive loop",
            //             next_receiver_data_port,
            //             producer,
            //         )?;
            //         Some(AudioPlayer::start_playback(
            //             desc.output_device_index,
            //             consumer,
            //         )?)
            //     }
            //     Switch::Disabled => None,
            // };

            // thread_loop::spawn("Pose data get loop", {
            //     let vr_client = vr_client.clone();
            //     move || {
            //         let (motion_data, input_device_data) = vr_client.lock().poll_input();
            //         let client_update = ClientUpdate {
            //             motion_data,
            //             input_device_data,
            //             vsync_offset_ns: 0, // todo
            //         };
            //         connection_manager
            //             .lock()
            //             .send_message_udp(&ClientMessage::Update(Box::new(client_update)))
            //             .map_err(|e| debug!("{}", e))
            //             .ok();
            //     }
            // })?;

            // compositor.lock().initialize_for_server();
            // vr_client.lock().initialize_for_server();

            // loop {}
            Ok(())
        }
    };

    trace_err!(thread::Builder::new()
        .name("Connection/statistics loop".into())
        .spawn(move || loop {
            show_err!(try_connect()).ok();
            vr_client.lock().deinitialize_for_server();
            compositor.lock().deinitialize_for_server();
        })
        .map(|_| ()))
}

pub fn entry_point() -> StrResult {
    logging_backend::init_logging();


    let graphics = Arc::new(GraphicsContext::new(None)?);
    let compositor = Arc::new(Mutex::new(Compositor::new(graphics.clone())?));
    let vr_client = Arc::new(Mutex::new(VrClient::new(graphics.clone())?));
    let connected_to_server = Arc::new(AtomicBool::new(false));

    begin_client_loop(
        compositor.clone(),
        vr_client.clone(),
        connected_to_server.clone(),
    )?;

    // todo check if rendering must be done on main thread
    loop {
        if connected_to_server.load(Ordering::Relaxed) {
            compositor.lock().render_idle_frame();
            vr_client.lock().submit_idle_frame();
        } else {
            compositor.lock().render_stream_frame();
            vr_client.lock().submit_stream_frame();
        }
    }
}

#[cfg(target_os = "android")]
fn android_entry_point() {
    // let app = ndk_glue::get_android_app();
    // show_err!(entry_point()).ok();
}

#[cfg(target_os = "android")]
ndk_glue::ndk_glue!(android_entry_point);