// use crate::{data::*, thread_loop::ThreadLoop, *};
// use laminar::{Config, LinkConditioner, Packet, Socket, SocketEvent};
// use log::*;
// use parking_lot::Mutex;
// use serde::{de::*, *};
// use std::{
//     cmp::*,
//     collections::*,
//     net::*,
//     sync::{mpsc::*, Arc},
//     time::*,
// };

// const TRACE_CONTEXT: &str = "Sockets";

// pub const MAX_HANDSHAKE_PACKET_SIZE_BYTES: usize = 4_000;

// const LOCAL_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
// const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);

// const HANDSHAKE_PORT: u16 = 9943;

// const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);

// #[derive(Serialize, Deserialize)]
// pub enum StreamType {
//     VideoSlice(u8),
//     GameAudio,
//     Microphone,

//     // Other types of streams don't have an ordering requirement and are collected by a single
//     // receiver. This is done to reduce the number of parallel threads needed.
//     // Haptic and shutdown for server; motion, input, statistics and disconnected for client
//     Other,
// }

// impl Into<u8> for StreamType {
//     fn into(self) -> u8 {
//         match self {
//             Self::Other => 0,
//             Self::GameAudio => 1,
//             Self::Microphone => 2,
//             Self::VideoSlice(idx) => 3 + idx,
//         }
//     }
// }

// pub fn search_client(
//     client_ip: Option<String>,
//     timeout: Duration,
// ) -> StrResult<(IpAddr, ClientHandshakePacket)> {
//     let deadline = Instant::now() + timeout;

//     let listener = trace_err!(UdpSocket::bind(SocketAddr::new(LOCAL_IP, HANDSHAKE_PORT)))?;
//     trace_err!(listener.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
//     trace_err!(listener.set_read_timeout(Some(HANDSHAKE_TIMEOUT)))?;

//     let maybe_target_client_ip = match client_ip {
//         Some(ip_str) => Some(trace_err!(ip_str.parse::<IpAddr>(), "Client IP")?),
//         None => None,
//     };

//     let mut packet_buffer = [0u8; MAX_HANDSHAKE_PACKET_SIZE_BYTES];
//     let mut try_find_client = || -> Result<(IpAddr, ClientHandshakePacket), ()> {
//         let (hanshake_packet_size, address) = listener
//             .recv_from(&mut packet_buffer)
//             .map_err(|e| debug!("No handshake packet received: {}", e))?;

//         if let Some(ip) = maybe_target_client_ip {
//             if address.ip() != ip {
//                 info!("Found client with wrong IP");
//                 return Err(());
//             }
//         }

//         let client_handshake_packet = bincode::deserialize(&packet_buffer[..hanshake_packet_size])
//             .map_err(|e| warn!("Received handshake packet: {}", e))?;

//         Ok((address.ip(), client_handshake_packet))
//     };

//     loop {
//         if let Ok(pair) = try_find_client() {
//             break Ok(pair);
//         } else if Instant::now() > deadline {
//             break Err("No valid client found".into());
//         }
//     }
// }

// pub enum SendMode {
//     UnreliableUnordered,
//     UnreliableSequential,
//     ReliableUnordered,
//     ReliableOrdered,
// }

// pub struct PacketEnqueuer {
//     peer_address: SocketAddr,
//     stream_id: u8,
//     send_mode: SendMode,
//     packet_sender: crossbeam_channel::Sender<Packet>,
// }

// impl PacketEnqueuer {
//     // todo: find a way to move the type parameter at struct level (issue with lifetimes)
//     pub fn enqueue<T: Serialize>(&mut self, packet: &T) -> StrResult {
//         // Laminar API takes ownership of the packet payloads so we need to reallocate new buffers
//         // for every send
//         let mut buffer = vec![self.stream_id];
//         // <&mut Vec>::write() appends the writtend data
//         trace_err!(bincode::serialize_into(&mut buffer, packet))?;

//         // todo: use const generics when stabilized
//         let packet = match self.send_mode {
//             SendMode::UnreliableUnordered => Packet::unreliable(self.peer_address, buffer),
//             SendMode::UnreliableSequential => {
//                 Packet::unreliable_sequenced(self.peer_address, buffer, Some(self.stream_id))
//             }
//             SendMode::ReliableUnordered => Packet::reliable_unordered(self.peer_address, buffer),
//             SendMode::ReliableOrdered => {
//                 Packet::reliable_ordered(self.peer_address, buffer, Some(self.stream_id))
//             }
//         };
//         trace_err!(self.packet_sender.send(packet))
//     }
// }

// pub struct ReceivedPacket {
//     buffer: Option<Vec<u8>>,
//     return_buffer_enqueuer: Sender<Vec<u8>>,
// }

// impl ReceivedPacket {
//     pub fn get<'a, T: Deserialize<'a>>(&'a self) -> StrResult<T> {
//         trace_err!(bincode::deserialize(&self.buffer.as_ref().unwrap()))
//     }
// }

// impl Drop for ReceivedPacket {
//     fn drop(&mut self) {
//         self.return_buffer_enqueuer
//             .send(self.buffer.take().unwrap())
//             .ok();
//     }
// }

// pub struct PacketDequeuer {
//     receive_buffer_dequeuer: Receiver<Vec<u8>>,
//     return_buffer_enqueuer: Sender<Vec<u8>>,
// }

// impl PacketDequeuer {
//     // todo: find a way to deserialize inside this function (issue with lifetimes)
//     pub fn dequeue(&mut self, timeout: Duration) -> StrResult<ReceivedPacket> {
//         let buffer = trace_err!(self.receive_buffer_dequeuer.recv_timeout(timeout))?;
//         Ok(ReceivedPacket {
//             buffer: Some(buffer),
//             return_buffer_enqueuer: self.return_buffer_enqueuer.clone(),
//         })
//     }
// }

// pub struct ConnectionManager {
//     peer_address: SocketAddr,
//     socket: Socket,
//     receive_thread: ThreadLoop,
//     receive_buffer_enqueuers: Arc<Mutex<HashMap<u8, Sender<Vec<u8>>>>>,
//     return_buffer_enqueuer: Sender<Vec<u8>>,
// }

// impl ConnectionManager {
//     fn create_config(socket_config: SocketConfig) -> Config {
//         let mut config = Config::default();
//         config.blocking_mode = false;
//         config.heartbeat_interval = None;

//         if let Some(value) = socket_config.idle_connection_timeout_ms {
//             config.idle_connection_timeout = Duration::from_millis(value);
//         }
//         if let Some(value) = socket_config.max_packet_size {
//             config.max_packet_size = value as _;
//         }
//         if let Some(value) = socket_config.max_fragments {
//             config.max_fragments = value;
//         }
//         if let Some(value) = socket_config.fragment_size {
//             config.fragment_size = value;
//         }
//         if let Some(value) = socket_config.fragment_reassembly_buffer_size {
//             config.fragment_reassembly_buffer_size = value;
//         }
//         if let Some(value) = socket_config.receive_buffer_max_size {
//             config.receive_buffer_max_size = value as _;
//         }
//         if let Some(value) = socket_config.rtt_smoothing_factor {
//             config.rtt_smoothing_factor = value;
//         }
//         if let Some(value) = socket_config.rtt_max_value {
//             config.rtt_max_value = value;
//         }
//         if let Some(value) = socket_config.socket_event_buffer_size {
//             config.socket_event_buffer_size = value as _;
//         }
//         if let Some(value) = socket_config.max_packets_in_flight {
//             config.max_packets_in_flight = value;
//         }

//         config
//     }

//     fn create_connection_manager(
//         local_address: SocketAddr,
//         peer_address: SocketAddr,
//         socket_config: SocketConfig,
//         mut timeout_callback: impl FnMut() + Send + 'static,
//     ) -> StrResult<Self> {
//         let config = Self::create_config(socket_config);
//         let mut socket = trace_err!(
//             Socket::bind_with_config(local_address, config),
//             "Handshake failed"
//         )?;

//         let (return_buffer_enqueuer, return_buffer_dequeuer) = channel::<Vec<_>>();
//         let event_receiver = socket.get_event_receiver();
//         let receive_buffer_enqueuers = Arc::new(Mutex::new(HashMap::<_, Sender<_>>::new()));
//         let receive_thread = thread_loop::spawn("Socket receiver loop", {
//             let receive_buffer_enqueuers = receive_buffer_enqueuers.clone();
//             move || {
//                 let mut buffer = if let Ok(mut buffer) = return_buffer_dequeuer.try_recv() {
//                     buffer.clear();
//                     buffer
//                 } else {
//                     vec![]
//                 };

//                 match event_receiver.recv() {
//                     Ok(SocketEvent::Packet(packet)) => {
//                         let payload = packet.payload();
//                         let stream_id = payload[0];
//                         buffer.extend(&payload[0..]);
//                         if let Some(enqueuer) = receive_buffer_enqueuers.lock().get(&stream_id) {
//                             enqueuer.send(buffer).ok();
//                         }
//                     }
//                     Ok(SocketEvent::Timeout(_)) => {
//                         timeout_callback();
//                     }
//                     _ => warn!("Unknown socket error"),
//                 }
//             }
//         })?;

//         Ok(ConnectionManager {
//             peer_address,
//             socket,
//             receive_thread,
//             receive_buffer_enqueuers,
//             return_buffer_enqueuer,
//         })
//     }

//     pub fn register_enqueuer(
//         &mut self,
//         stream_type: StreamType,
//         send_mode: SendMode,
//     ) -> PacketEnqueuer {
//         let packet_sender = self.socket.get_packet_sender();
//         PacketEnqueuer {
//             peer_address: self.peer_address,
//             stream_id: stream_type.into(),
//             send_mode,
//             packet_sender,
//         }
//     }

//     pub fn register_dequeuer(&mut self, stream_type: StreamType) -> PacketDequeuer {
//         let (receive_buffer_enqueuer, receive_buffer_dequeuer) = channel();

//         self.receive_buffer_enqueuers
//             .lock()
//             .insert(stream_type.into(), receive_buffer_enqueuer);

//         PacketDequeuer {
//             receive_buffer_dequeuer,
//             return_buffer_enqueuer: self.return_buffer_enqueuer.clone(),
//         }
//     }

//     pub fn enable_debug(&mut self, packet_loss_rate: Option<f64>, latency: Option<Duration>) {
//         let mut conditioner = LinkConditioner::new();

//         if let Some(packet_loss_rate) = packet_loss_rate {
//             conditioner.set_packet_loss(packet_loss_rate);
//         }
//         if let Some(latency) = latency {
//             conditioner.set_latency(latency);
//         }

//         self.socket.set_link_conditioner(Some(conditioner));
//     }

//     pub fn connect_to_client(
//         found_client_ip: IpAddr,
//         socket_config: SocketConfig,
//         handshake_packet: ServerHandshakePacket,
//         timeout_callback: impl FnMut() + Send + 'static,
//     ) -> StrResult<Self> {
//         let handshake_server_address = SocketAddr::new(LOCAL_IP, HANDSHAKE_PORT);
//         let client_address = SocketAddr::new(
//             found_client_ip,
//             handshake_packet.settings.connection.client_port,
//         );

//         let hanshake_sender = trace_err!(
//             TcpStream::connect(handshake_server_address),
//             "Handshake failed"
//         )?;

//         trace_err!(bincode::serialize_into(hanshake_sender, &handshake_packet))?;
//         // hanshake_sender dropped here. Close TCP connection because it can interfere with Laminar

//         let server_address =
//             SocketAddr::new(LOCAL_IP, handshake_packet.settings.connection.server_port);
//         Self::create_connection_manager(
//             server_address,
//             client_address,
//             socket_config,
//             timeout_callback,
//         )
//     }

//     pub fn connect_to_server(
//         handshake_packet: ClientHandshakePacket,
//         timeout_callback: impl FnMut() + Send + 'static,
//     ) -> StrResult<(Self, ServerHandshakePacket)> {
//         let multicaster = trace_err!(UdpSocket::bind(SocketAddr::new(LOCAL_IP, HANDSHAKE_PORT)))?;
//         trace_err!(multicaster.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
//         trace_err!(multicaster.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))?;

//         let listener = trace_err!(TcpListener::bind(SocketAddr::new(LOCAL_IP, HANDSHAKE_PORT)))?;
//         trace_err!(listener.set_nonblocking(true))?;

//         let client_hanshake_packet = trace_err!(bincode::serialize(&handshake_packet))?;

//         let try_handshake = || -> Result<(IpAddr, ServerHandshakePacket), ()> {
//             multicaster
//                 .send_to(
//                     &client_hanshake_packet,
//                     SocketAddr::V4(SocketAddrV4::new(MULTICAST_ADDR, HANDSHAKE_PORT)),
//                 )
//                 .map_err(|err| debug!("Handshake packet multicast: {}", err))?;

//             let accept_deadline = Instant::now() + HANDSHAKE_TIMEOUT;
//             let (handshake_receiver, address) = loop {
//                 if let Ok(pair) = listener.accept() {
//                     break pair;
//                 } else if Instant::now() > accept_deadline {
//                     return Err(());
//                 }
//             };
//             handshake_receiver
//                 .set_nonblocking(false)
//                 .map_err(|err| warn!("Control socket: {}", err))?;

//             let server_handshake_packet = bincode::deserialize_from(handshake_receiver)
//                 .map_err(|err| warn!("Handshake packet receive: {}", err))?;
//             // handshake_receiver dropped here. Close TCP connection because it can interfere with
//             // Laminar

//             Ok((address.ip(), server_handshake_packet))
//         };

//         let (server_ip, server_handshake_packet) = loop {
//             if let Ok(server_candidate) = try_handshake() {
//                 break server_candidate;
//             }
//         };

//         let client_address = SocketAddr::new(
//             LOCAL_IP,
//             server_handshake_packet.settings.connection.client_port,
//         );
//         let server_address = SocketAddr::new(
//             server_ip,
//             server_handshake_packet.settings.connection.server_port,
//         );

//         let connection_manager = Self::create_connection_manager(
//             client_address,
//             server_address,
//             server_handshake_packet.settings.connection.config.clone(),
//             timeout_callback,
//         )?;

//         Ok((connection_manager, server_handshake_packet))
//     }

//     pub fn request_stop(&mut self) {
//         self.receive_thread.request_stop();
//     }
// }
