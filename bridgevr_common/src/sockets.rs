use crate::{packets::*, ring_buffer::*, thread_loop::ThreadLoop, *};
use log::*;
use serde::{de::*, *};
use std::{collections::*, convert::TryInto, marker::PhantomData, net::*, sync::*, time::*};

const MAX_PACKET_SIZE_BYTES: usize = 4_000;

// this is used with UDP for pose data and TCP for shutdown signal
// at least other two ports are used (out p1: video, out p2: audio, in p1: microphone)
pub const MESSAGE_PORT: u16 = 9943;

const BIND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED); // todo: or Ipv4Addr::LOCALHOST ?
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);
const PACKET_TIMEOUT: Duration = Duration::from_secs(1);

const TRACE_CONTEXT: &str = "Sockets";

fn create_udp_socket(peer_ip: IpAddr, port: u16) -> StrResult<UdpSocket> {
    let udp_message_socket = trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, port)))?;
    trace_err!(udp_message_socket.connect(SocketAddr::new(peer_ip, port)))?;
    trace_err!(udp_message_socket.set_read_timeout(Some(PACKET_TIMEOUT)))?;
    trace_err!(udp_message_socket.set_write_timeout(Some(PACKET_TIMEOUT)))?;
    Ok(udp_message_socket)
}

pub fn get_data_offset<H: Serialize>(header: &H) -> usize {
    bincode::serialized_size(header).unwrap() as usize + 8 // 8 is index byte size
}

pub fn serialize_indexed_header_into<H: Serialize>(
    buffer: &mut Vec<u8>,
    index: u64,
    header: &H,
) -> StrResult<()> {
    // first 8 bytes are the index
    buffer[..8].copy_from_slice(&index.to_le_bytes());
    trace_err!(bincode::serialize_into(&mut buffer[8..], header))
}

pub fn deserialize_indexed_header_from<H: DeserializeOwned>(buffer: &[u8]) -> StrResult<H> {
    trace_err!(bincode::deserialize_from(buffer))
}

pub struct SenderData {
    packet: Vec<u8>,
    header_size: usize,
    data_size: usize,
}

// metadata is any information relative to the packet that is not stored directly in it.
// In particular it is used as input index by MediaCodec.
pub struct ReceiverData<M> {
    packet: Vec<u8>,
    metadata: M,
    packet_size: usize,
}

struct SocketData {
    socket: Arc<UdpSocket>,
    sender_thread: Option<ThreadLoop>,
    receiver_thread: Option<ThreadLoop>,
}

pub struct ConnectionManager<SM> {
    peer_ip: IpAddr,
    udp_message_socket: Arc<UdpSocket>,
    tcp_message_socket: Arc<TcpStream>,
    udp_message_receiver_thread: ThreadLoop,
    tcp_message_receiver_thread: ThreadLoop,
    buffer_sockets: HashMap<u16, SocketData>,

    // Vec<u8> specifically implements Write. Written data is appended.
    // Must be cleared before sending new data.
    // todo: remove Arc<Mutex<>>?
    send_message_buffer: Arc<Mutex<Vec<u8>>>,

    //this phantom data forces a ConnectionManager instance to always send the same type of data.
    // It has size 0 and is just a hint for the compiler
    phantom: PhantomData<SM>,
}

pub struct ClientCandidateDesc {
    client_ip: IpAddr,
    tcp_message_socket: TcpStream,
}

#[derive(Serialize, Deserialize)]
struct ServerHandshakeWrapper(ServerHandshakePacket, u16);

impl<SM> ConnectionManager<SM> {
    fn create_connection_manager<R: DeserializeOwned + 'static>(
        tcp_message_socket: TcpStream,
        peer_ip: IpAddr,
        message_port: u16,
        message_received_callback: Arc<Mutex<dyn FnMut(R) + Send>>,
    ) -> StrResult<Self> {
        let udp_message_socket = Arc::new(create_udp_socket(peer_ip, message_port)?);
        let tcp_message_socket = Arc::new(tcp_message_socket);

        let udp_message_receiver_thread = thread_loop::spawn({
            let message_received_callback = message_received_callback.clone();
            let udp_message_socket = udp_message_socket.clone();
            let mut packet_buffer = [0; MAX_PACKET_SIZE_BYTES];

            let mut try_receive = move || -> UnitResult {
                let size = udp_message_socket.recv(&mut packet_buffer).map_err(|e| {
                    warn!("UDP message receive: {}", e);
                })?;

                let message = bincode::deserialize(&packet_buffer[..size])
                    .map_err(|e| warn!("Received message: {}", e))?;

                (&mut *message_received_callback.lock().unwrap())(message);

                Ok(())
            };

            move || {
                try_receive().ok();
            }
        });

        let tcp_message_receiver_thread = thread_loop::spawn({
            let tcp_message_socket = tcp_message_socket.clone();

            move || match bincode::deserialize_from(&*tcp_message_socket) {
                Ok(message) => (&mut *message_received_callback.lock().unwrap())(message),
                Err(err) => {
                    warn!("TCP message receive: {}", err);
                    //todo: shutdown
                }
            }
        });

        Ok(ConnectionManager {
            peer_ip,
            udp_message_socket,
            tcp_message_socket,
            udp_message_receiver_thread,
            tcp_message_receiver_thread,
            buffer_sockets: HashMap::new(),
            send_message_buffer: Arc::default(),

            phantom: PhantomData,
        })
    }

    pub fn search_client(
        client_ip: Option<String>,
        timeout: Duration,
    ) -> StrResult<(ClientHandshakePacket, ClientCandidateDesc)> {
        let deadline = Instant::now() + timeout;

        let listener = trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, MESSAGE_PORT)))?;
        trace_err!(listener.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
        trace_err!(listener.set_read_timeout(Some(HANDSHAKE_TIMEOUT)))?;

        let maybe_target_client_ip = match &client_ip {
            Some(ip_str) => Some(trace_err!(ip_str.parse::<IpAddr>(), "Client IP")?),
            None => None,
        };

        let mut packet_buffer = [0u8; MAX_PACKET_SIZE_BYTES];
        let mut try_find_client = || -> Result<(ClientHandshakePacket, ClientCandidateDesc), ()> {
            let (size, address) = listener
                .recv_from(&mut packet_buffer)
                .map_err(|e| warn!("No handshake packet received: {}", e))?;

            if let Some(ip) = maybe_target_client_ip {
                if address.ip() != ip {
                    warn!("Found client with wrong IP");
                    Err(())?;
                }
            }

            let client_handshake_packet = bincode::deserialize(&packet_buffer[..size])
                .map_err(|e| warn!("Received handshake packet: {}", e))?;
            let tcp_message_socket =
                TcpStream::connect(address).map_err(|e| warn!("TCP connection: {}", e))?;

            Ok((
                client_handshake_packet,
                ClientCandidateDesc {
                    client_ip: address.ip(),
                    tcp_message_socket,
                },
            ))
        };

        loop {
            if let Ok(pair) = try_find_client() {
                break Ok(pair);
            } else if Instant::now() > deadline {
                break Err("No valid client found".into());
            }
        }
    }

    pub fn connect_to_client(
        client_candidate_desc: ClientCandidateDesc,
        starting_data_port: u16,
        handshake_packet: ServerHandshakePacket,
        message_received_callback: impl FnMut(ClientMessage) + Send + 'static,
    ) -> StrResult<Self> {
        let message_received_callback = Arc::new(Mutex::new(message_received_callback));
        trace_err!(
            bincode::serialize_into(
                &client_candidate_desc.tcp_message_socket,
                &ServerHandshakeWrapper(handshake_packet, starting_data_port),
            ),
            "Handshake packet send"
        )?;

        Self::create_connection_manager(
            client_candidate_desc.tcp_message_socket,
            client_candidate_desc.client_ip,
            starting_data_port,
            message_received_callback,
        )
    }

    // No timeout, the client always listens for server until a successful connection.
    pub fn connect_to_server(
        handshake_packet: ClientHandshakePacket,
        message_received_callback: impl FnMut(ServerMessage) + Send + 'static,
    ) -> StrResult<(Self, ServerHandshakePacket)> {
        // Because it is consumed in the loop, the callback must be clonable, but then I need Mutex
        // to preserve mutability. Because of this, create_connection_manager accepts an Arc<Mutex>.
        let message_received_callback = Arc::new(Mutex::new(message_received_callback));

        let multicaster = trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, MESSAGE_PORT)))?;
        trace_err!(multicaster.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
        trace_err!(multicaster.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))?;

        let listener = trace_err!(TcpListener::bind(SocketAddr::new(BIND_ADDR, MESSAGE_PORT)))?;
        trace_err!(listener.set_nonblocking(true))?;

        let client_hanshake_packet = bincode::serialize(&handshake_packet).unwrap();

        let try_handshake = || -> Result<(Self, ServerHandshakePacket), ()> {
            multicaster
                .send_to(
                    &client_hanshake_packet,
                    SocketAddr::V4(SocketAddrV4::new(MULTICAST_ADDR, MESSAGE_PORT)),
                )
                .map_err(|err| warn!("Handshake packet multicast: {}", err))?;

            let accept_deadline = Instant::now() + HANDSHAKE_TIMEOUT;
            let (control_socket, address) = loop {
                if let Ok(pair) = listener.accept() {
                    break pair;
                } else if Instant::now() > accept_deadline {
                    Err(())?;
                }
            };
            control_socket
                .set_nonblocking(false)
                .map_err(|err| warn!("Control socket: {}", err))?;

            let ServerHandshakeWrapper(server_handshake_packet, message_port) =
                bincode::deserialize_from(&control_socket)
                    .map_err(|err| warn!("Handshake packet receive: {}", err))?;

            let connection_manager = Self::create_connection_manager(
                control_socket,
                address.ip(),
                message_port,
                message_received_callback.clone(),
            )
            .map_err(|_| warn!("Cannot create connection manager"))?;

            Ok((connection_manager, server_handshake_packet))
        };

        loop {
            if let Ok(connection_manager) = try_handshake() {
                break Ok(connection_manager);
            }
        }
    }

    pub fn begin_send_indexed_buffers(
        &mut self,
        port: u16,
        mut buffer_consumer: Consumer<SenderData>,
    ) -> StrResult<()> {
        let socket_data_ref = self.buffer_sockets.entry(port).or_insert(SocketData {
            socket: Arc::new(create_udp_socket(self.peer_ip, port)?),
            sender_thread: None,
            receiver_thread: None,
        });

        if socket_data_ref.sender_thread.is_some() {
            Err(format!("Already sending on port {}", port))?;
        }

        let socket = socket_data_ref.socket.clone();
        socket_data_ref.sender_thread = Some(thread_loop::spawn(move || {
            buffer_consumer
                .consume(PACKET_TIMEOUT, |data| -> UnitResult {
                    // todo: send returns a usize. check that the whole packet is sent
                    socket
                        .send(&data.packet[0..(data.header_size + data.data_size)])
                        .map(|_| ())
                        .map_err(|e| warn!("UDP send error: {}", e))
                })
                .ok();
        }));

        Ok(())
    }

    pub fn begin_receive_indexed_buffers<M: Send + 'static>(
        &mut self,
        port: u16,
        mut buffer_producer: Producer<ReceiverData<M>, u64>,
    ) -> StrResult<()> {
        let socket_data_ref = self.buffer_sockets.entry(port).or_insert(SocketData {
            socket: Arc::new(create_udp_socket(self.peer_ip, port)?),
            sender_thread: None,
            receiver_thread: None,
        });

        if socket_data_ref.receiver_thread.is_some() {
            Err(format!("Already listening on port {}", port))?;
        }

        let socket = socket_data_ref.socket.clone();
        socket_data_ref.receiver_thread = Some(thread_loop::spawn(move || {
            buffer_producer
                .fill(PACKET_TIMEOUT, |data| -> Result<u64, ()> {
                    data.packet_size = socket.recv(&mut data.packet).map_err(|err| {
                        warn!("UDP buffer receive: {}", err);
                    })?;

                    // extract packet index
                    Ok(u64::from_le_bytes(
                        (&data.packet as &[u8]).try_into().unwrap(),
                    ))
                })
                .ok();
        }));

        Ok(())
    }
}

impl<SM: Serialize> ConnectionManager<SM> {
    pub fn send_message_udp(&self, packet: &SM) {
        // reuse same buffer to avoid unnecessary reallocations
        let mut send_message_buffer = self.send_message_buffer.lock().unwrap();
        send_message_buffer.clear();

        let packet_size = bincode::serialized_size(packet).unwrap();
        bincode::serialize_into(&mut *send_message_buffer, packet).unwrap();

        //todo: send() returns a usize. Check that the whole packet is sent in one go
        if let Err(err) = self
            .udp_message_socket
            .send(&send_message_buffer[..packet_size as _])
        {
            warn!("UDP send error: {}", err);
        }
    }

    pub fn send_message_tcp(&mut self, packet: &SM) {
        if let Err(err) = bincode::serialize_into(&*self.tcp_message_socket, packet) {
            warn!("TCP send error: {}", err);
        }
    }
}

impl<SM> Drop for ConnectionManager<SM> {
    fn drop(&mut self) {
        // by calling request_stop() before drop, which is non blocking, I avoid waiting
        // PACKET_TIMEOUT for every thread.
        self.udp_message_receiver_thread.request_stop();
        self.tcp_message_receiver_thread.request_stop();

        for (
            _,
            SocketData {
                sender_thread,
                receiver_thread,
                ..
            },
        ) in &mut self.buffer_sockets
        {
            sender_thread.as_ref().map(|t| t.request_stop());
            receiver_thread.as_ref().map(|t| t.request_stop());
        }
    }
}