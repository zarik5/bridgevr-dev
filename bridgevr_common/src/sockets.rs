use crate::{packets::*, settings::*, *};
use log::*;
use serde::{de::*, *};
use std::{
    collections::*,
    marker::PhantomData,
    net::*,
    sync::{mpsc, *},
    thread::{self, JoinHandle},
    time::*,
};

const MAX_PACKET_SIZE_BYTES: usize = 4_000;

pub const CONTROL_PORT: u16 = 9943;

const BIND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED); // todo: or Ipv4Addr::LOCALHOST ?
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);
const PACKET_TIMEOUT: Duration = Duration::from_secs(1);

const CONTEXT: &str = "Sockets";
macro_rules! trace_err {
    ($res:expr $(, $expect:expr)?) => {
        crate::trace_err!($res, CONTEXT $(, $expect)?)
    };
}

enum ThreadMessage<T> {
    Data(T),
    Shutdown,
}

struct ThreadHandles<T> {
    join_handle: JoinHandle<StrResult<()>>,
    message_sender: mpsc::Sender<ThreadMessage<T>>,
}

fn create_udp_socket(peer_ip: IpAddr, port: u16) -> StrResult<UdpSocket> {
    let udp_message_socket = trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, port)))?;
    trace_err!(udp_message_socket.connect(SocketAddr::new(peer_ip, port)))?;
    trace_err!(udp_message_socket.set_read_timeout(Some(PACKET_TIMEOUT)))?;
    trace_err!(udp_message_socket.set_write_timeout(Some(PACKET_TIMEOUT)))?;
    Ok(udp_message_socket)
}

pub struct BufferSenderSocket<H> {
    socket: Arc<UdpSocket>,
    header_buffer: Vec<u8>,

    phantom_header: PhantomData<H>,
}

// Prepare a packet made of a header and a buffer and send it with a socket without copying the
// buffer. The header is copied two times.
impl<H: Serialize> BufferSenderSocket<H> {
    pub fn get_buffer_offset(header: &H) -> usize {
        // reserve 8 more bytes for the buffer size
        bincode::serialized_size(header).unwrap() as usize
    }

    // packet_buffer should be of size buffer_offset + buffer_size
    pub fn send_packet(&mut self, header: &H, packet_buffer: &mut [u8]) -> StrResult<()> {
        self.header_buffer.clear();
        bincode::serialize_into(&mut self.header_buffer, header).unwrap();
        let header_size = self.header_buffer.len();
        if packet_buffer.len() >= header_size {
            packet_buffer[..header_size].copy_from_slice(&self.header_buffer);

            //todo: send() returns a usize. Check that the whole packet is sent in one go
            if let Err(err) = self.socket.send(packet_buffer) {
                warn!("UDP send error: {}", err);
            }
            Ok(())
        } else {
            Err("Packet buffer is too small".to_owned())
        }
    }
}

// Receive a packet made of a header and a bufferwithout copying the buffer. The header is copied
// one time.
pub struct BufferReceiverSocket<K> {
    thread_message_sender: mpsc::Sender<ThreadMessage<(K, Arc<Mutex<[u8]>>)>>,
}

impl<K: Clone> BufferReceiverSocket<K> {
    pub fn enqueue_buffer(&mut self, key: K, buffer: Arc<Mutex<[u8]>>) -> Result<(), K> {
        self.thread_message_sender
            .send(ThreadMessage::Data((key.clone(), buffer)))
            .map_err(|_| key)
    }
}

pub struct ConnectionManager<SM, K> {
    peer_ip: IpAddr,
    udp_message_socket: Arc<UdpSocket>,
    tcp_message_socket: Arc<TcpStream>,
    udp_message_receive_thread_handles: Option<ThreadHandles<()>>,
    tcp_message_receive_thread_handles: Option<ThreadHandles<()>>,
    udp_buffer_sockets:
        HashMap<u16, (Arc<UdpSocket>, Option<ThreadHandles<(K, Arc<Mutex<[u8]>>)>>)>,
    shutdown_callback: Arc<Mutex<dyn FnMut() + Send>>,
    bincode_config: Arc<bincode::Config>,

    // Vec<u8> specifically implements Write. Written data is appended.
    // Must be cleared before sending new data.
    send_message_buffer: Arc<Mutex<Vec<u8>>>,

    //this phantom data forces a ConnectionManager instance to always send the same type of data.
    // It has size 0 and is just a hint for the compiler
    phantom_send_packet: PhantomData<SM>,
}

impl<SM, K> ConnectionManager<SM, K> {
    pub fn get_send_buffer_socket<H>(&mut self, port: u16) -> StrResult<BufferSenderSocket<H>> {
        let (socket_ref, _) = self
            .udp_buffer_sockets
            .entry(port)
            .or_insert((Arc::new(create_udp_socket(self.peer_ip, port)?), None));
        Ok(BufferSenderSocket {
            socket: socket_ref.clone(),
            header_buffer: vec![],
            phantom_header: PhantomData,
        })
    }

    pub fn shutdown(&mut self) {
        if let Some(ThreadHandles {
            join_handle,
            message_sender,
        }) = self.udp_message_receive_thread_handles.take()
        {
            message_sender.send(ThreadMessage::Shutdown).ok();
            join_handle.join().ok();
        }

        if let Some(ThreadHandles {
            join_handle,
            message_sender,
        }) = self.tcp_message_receive_thread_handles.take()
        {
            message_sender.send(ThreadMessage::Shutdown).ok();
            join_handle.join().ok();
        }

        for (_, (_, maybe_thread_handle)) in &mut self.udp_buffer_sockets {
            if let Some(ThreadHandles {
                join_handle,
                message_sender,
            }) = maybe_thread_handle.take()
            {
                message_sender.send(ThreadMessage::Shutdown).ok();
                join_handle.join().ok();
            }
        }
    }
}

impl<SM: Serialize, K> ConnectionManager<SM, K> {
    pub fn send_message_udp(&self, packet: &SM) {
        // reuse same buffer to avoid unnecessary reallocations
        let mut send_message_buffer = self.send_message_buffer.lock().unwrap();
        send_message_buffer.clear();

        let packet_size = self.bincode_config.serialized_size(packet).unwrap();
        self.bincode_config
            .serialize_into(&mut *send_message_buffer, packet)
            .unwrap();

        //todo: send() returns a usize. Check that the whole packet is sent in one go
        if let Err(err) = self
            .udp_message_socket
            .send(&send_message_buffer[..packet_size as _])
        {
            warn!("UDP send error: {}", err);
        }
    }

    pub fn send_message_tcp(&mut self, packet: &SM) {
        if let Err(err) = self
            .bincode_config
            .serialize_into(&*self.tcp_message_socket, packet)
        {
            warn!("TCP send error: {}", err);
            self.shutdown();
        }
    }
}

impl<SM, K: Send + 'static> ConnectionManager<SM, K> {
    // returns: submit buffer callback
    pub fn get_receive_buffer_socket<H: DeserializeOwned + Serialize>(
        &mut self,
        port: u16,
        mut receive_callback: impl FnMut(&K, Option<(H, usize)>) + Send + 'static, // (header, key, buffer size)
    ) -> StrResult<BufferReceiverSocket<K>> {
        let (socket_ref, thread_handles_ref) = self
            .udp_buffer_sockets
            .entry(port)
            .or_insert((Arc::new(create_udp_socket(self.peer_ip, port)?), None));

        if thread_handles_ref.is_none() {
            let (thread_message_sender, thread_message_receiver) = mpsc::channel();

            let join_handle = {
                let socket = socket_ref.clone();
                thread::spawn(move || -> StrResult<()> {
                    let mut try_receive = |key: &K, buffer: &Arc<Mutex<[u8]>>| -> Result<(), ()> {
                        let mut buffer = buffer.lock().unwrap();
                        let packet_size = socket.recv(&mut buffer).map_err(|err| {
                            warn!("UDP buffer receive: {}", err);
                        })?;
                        // Read is implemented for &[u8] by copying data.
                        // todo: check that only the read part (the header) is copied.
                        let header = bincode::deserialize_from(&buffer[..packet_size])
                            .map_err(|err| warn!("Read packet header: {}", err))?;
                        // todo: check performance of serialized_size()
                        let header_size = bincode::serialized_size(&header).unwrap() as usize;

                        receive_callback(key, Some((header, packet_size - header_size)));

                        Ok(())
                    };

                    let mut buffer_queue = VecDeque::new();
                    let mut running = true;
                    while running {
                        while let Some((key, buffer)) = buffer_queue.front() {
                            if try_receive(key, buffer).is_ok() {
                                buffer_queue.pop_front();
                            }
                        }

                        let mut maybe_message = thread_message_receiver
                            .recv_timeout(PACKET_TIMEOUT)
                            .map_err(|_| ());
                        while let Ok(message) = maybe_message {
                            match message {
                                ThreadMessage::Data(keyed_buffer) => {
                                    buffer_queue.push_back(keyed_buffer)
                                }
                                ThreadMessage::Shutdown => running = false,
                            }

                            maybe_message = thread_message_receiver.try_recv().map_err(|_| ());
                        }
                    }

                    Ok(())
                })
            };

            *thread_handles_ref = Some(ThreadHandles {
                join_handle,
                message_sender: thread_message_sender.clone(),
            });

            Ok(BufferReceiverSocket {
                thread_message_sender,
            })
        } else {
            Err(format!("Already listening on port {}", port))
        }
    }
}

impl<SM, K> Drop for ConnectionManager<SM, K> {
    fn drop(&mut self) {
        self.shutdown();
        (&mut *self.shutdown_callback.lock().unwrap())();
    }
}

fn create_connection_socket<SM, R: DeserializeOwned + 'static, K>(
    tcp_message_socket: TcpStream,
    peer_ip: IpAddr,
    message_port: u16,
    message_received_callback: Arc<Mutex<dyn FnMut(R) + Send>>,
    shutdown_callback: impl FnMut() + Send + 'static,
) -> StrResult<ConnectionManager<SM, K>> {
    let tcp_message_socket = Arc::new(tcp_message_socket);
    let shutdown_callback = Arc::new(Mutex::new(shutdown_callback));

    let bincode_config = Arc::new(bincode::config());
    let udp_message_socket = Arc::new(create_udp_socket(peer_ip, message_port)?);

    let (udp_thread_message_sender, udp_thread_message_receiver) = mpsc::channel();

    let udp_join_handle = {
        let message_received_callback = message_received_callback.clone();
        let udp_message_socket = udp_message_socket.clone();
        let bincode_config = bincode_config.clone();
        thread::spawn(move || -> StrResult<()> {
            let mut packet_buffer = [0; MAX_PACKET_SIZE_BYTES];

            let mut try_receive = || -> Result<(), ()> {
                let size = udp_message_socket.recv(&mut packet_buffer).map_err(|err| {
                    warn!("UDP message receive: {}", err);
                })?;

                let message = bincode_config
                    .deserialize(&packet_buffer[..size])
                    .map_err(|err| warn!("Received message: {}", err))?;

                (&mut *message_received_callback.lock().unwrap())(message);

                Ok(())
            };

            loop {
                try_receive().ok();

                if let Ok(message) = udp_thread_message_receiver.try_recv() {
                    if let ThreadMessage::Shutdown = message {
                        break;
                    }
                }
            }

            Ok(())
        })
    };

    let (tcp_thread_message_sender, tcp_thread_message_receiver) = mpsc::channel();

    let tcp_join_handle = {
        let message_received_callback = message_received_callback.clone();
        let tcp_message_socket = tcp_message_socket.clone();
        let bincode_config = bincode_config.clone();
        let shutdown_callback = shutdown_callback.clone();
        thread::spawn(move || -> StrResult<()> {
            loop {
                match bincode_config.deserialize_from(&*tcp_message_socket) {
                    Ok(message) => (&mut *message_received_callback.lock().unwrap())(message),
                    Err(err) => {
                        warn!("TCP message receive: {}", err);
                        (&mut *shutdown_callback.lock().unwrap())();
                    }
                }

                if let Ok(message) = tcp_thread_message_receiver.try_recv() {
                    if let ThreadMessage::Shutdown = message {
                        break;
                    }
                }
            }
            Ok(())
        })
    };

    Ok(ConnectionManager {
        peer_ip,
        udp_message_socket,
        tcp_message_socket,
        udp_message_receive_thread_handles: Some(ThreadHandles {
            join_handle: udp_join_handle,
            message_sender: udp_thread_message_sender,
        }),
        tcp_message_receive_thread_handles: Some(ThreadHandles {
            join_handle: tcp_join_handle,
            message_sender: tcp_thread_message_sender,
        }),
        udp_buffer_sockets: HashMap::new(),
        shutdown_callback,
        send_message_buffer: <_>::default(),
        bincode_config,

        phantom_send_packet: PhantomData,
    })
}

#[derive(Serialize, Deserialize)]
struct ServerHandshakeWrapper((ServerHandshakePacket, u16));

pub struct ConnectionDesc<ConnectionCallback, MessageReceivedCallback> {
    handshake_packet: ServerHandshakePacket,
    connection_callback: ConnectionCallback,
    message_received_callback: MessageReceivedCallback,
}

pub struct HandshakeSocket {
    thread_handles: Option<ThreadHandles<()>>,
}

impl HandshakeSocket {
    pub fn start_listening<CC, MRC, K>(
        connections: Connections,
        mut client_found_callback: impl FnMut(ClientHandshakePacket) -> StrResult<ConnectionDesc<CC, MRC>>
            + Send
            + 'static,
    ) -> Self
    where
        CC: FnOnce(ConnectionManager<ServerMessage, K>) -> StrResult<()>,
        MRC: FnMut(ClientMessage) + Send + 'static,
    {
        let (thread_message_sender, thread_message_receiver) = mpsc::channel();
        let join_handle = {
            let thread_message_sender = thread_message_sender.clone();
            thread::spawn(move || -> StrResult<()> {
                let mut connected_clients_count = 0;
                let max_clients = match connections.clients {
                    Clients::Count(count) => count as i32,
                    Clients::WithIp(_) => 1,
                };

                let listener =
                    trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, CONTROL_PORT)))?;
                trace_err!(listener.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
                trace_err!(listener.set_read_timeout(Some(HANDSHAKE_TIMEOUT)))?;

                let maybe_target_client_ip = match &connections.clients {
                    Clients::WithIp(ip) => Some(trace_err!(ip.parse::<IpAddr>(), "Client IP")?),
                    _ => None,
                };

                let mut packet_buffer = [0u8; MAX_PACKET_SIZE_BYTES];

                // use closure + `?` operator for early return
                let mut try_handshake = |connected_clients_count: &mut i32| -> Result<(), ()> {
                    let (size, address) = listener
                        .recv_from(&mut packet_buffer)
                        .map_err(|err| info!("No handshake packet received: {}", err))?;
                    if let Some(ip) = maybe_target_client_ip {
                        if address.ip() != ip {
                            Err(())?;
                        }
                    }
                    let client_handshake_packet = bincode::deserialize(&packet_buffer[..size])
                        .map_err(|err| warn!("Handshake packet receive: {}", err))?;
                    let tcp_message_socket = TcpStream::connect(address)
                        .map_err(|err| warn!("TCP connection failed: {}", err))?;
                    let connection_desc =
                        display_err!(client_found_callback(client_handshake_packet))?;
                    bincode::serialize_into(
                        &tcp_message_socket,
                        &ServerHandshakeWrapper((
                            connection_desc.handshake_packet,
                            connections.starting_data_port,
                        )),
                    )
                    .map_err(|err| warn!("Handshake packet send: {}", err))?;

                    let thread_message_sender = thread_message_sender.clone();
                    display_err!((connection_desc.connection_callback)(display_err!(
                        create_connection_socket(
                            tcp_message_socket,
                            address.ip(),
                            connections.starting_data_port,
                            Arc::new(Mutex::new(connection_desc.message_received_callback)),
                            move || {
                                thread_message_sender.send(ThreadMessage::Data(())).ok();
                            },
                        )
                    )?))?;
                    *connected_clients_count += 1;

                    Ok(())
                };

                loop {
                    let maybe_message = if connected_clients_count < max_clients {
                        try_handshake(&mut connected_clients_count).ok();
                        // check for shutdown (non blocking)
                        thread_message_receiver.try_recv().ok()
                    } else {
                        // wait for disconnected clients or shutdown
                        thread_message_receiver.recv().ok()
                    };

                    if let Some(message) = maybe_message {
                        match message {
                            ThreadMessage::Data(()) => connected_clients_count -= 1,
                            ThreadMessage::Shutdown => break,
                        }
                    }
                }

                Ok(())
            })
        };

        Self {
            thread_handles: Some(ThreadHandles {
                join_handle,
                message_sender: thread_message_sender,
            }),
        }
    }

    pub fn start_multicasting<K>(
        handshake_packet: ClientHandshakePacket,
        mut connection_callback: impl FnMut(ConnectionManager<ClientMessage, K>, ServerHandshakePacket) -> StrResult<()>
            + Send
            + 'static,
        message_receive_callback: impl FnMut(ServerMessage) + Send + 'static,
    ) -> Self {
        let (thread_message_sender, thread_message_receiver) = mpsc::channel();
        let join_handle = {
            let thread_message_sender = thread_message_sender.clone();
            thread::spawn(move || -> StrResult<()> {
                let message_receive_callback = Arc::new(Mutex::new(message_receive_callback));

                let multicaster =
                    trace_err!(UdpSocket::bind(SocketAddr::new(BIND_ADDR, CONTROL_PORT)))?;
                trace_err!(multicaster.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED))?;
                trace_err!(multicaster.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))?;

                let listener =
                    trace_err!(TcpListener::bind(SocketAddr::new(BIND_ADDR, CONTROL_PORT)))?;
                trace_err!(listener.set_nonblocking(true))?;

                let client_hanshake_packet = bincode::serialize(&handshake_packet).unwrap();

                let mut try_handshake = || -> Result<(), ()> {
                    multicaster
                        .send_to(
                            &client_hanshake_packet,
                            SocketAddr::V4(SocketAddrV4::new(MULTICAST_ADDR, CONTROL_PORT)),
                        )
                        .map_err(|err| warn!("Handshake packet multicast: {}", err))?;

                    let accept_timeout_time = Instant::now() + HANDSHAKE_TIMEOUT;
                    let (control_socket, address) = loop {
                        if let Ok(res) = listener.accept() {
                            break res;
                        } else if Instant::now() > accept_timeout_time {
                            Err(())?;
                        }
                    };
                    control_socket
                        .set_nonblocking(false)
                        .map_err(|err| warn!("Control socket: {}", err))?;

                    let ServerHandshakeWrapper((server_handshake_packet, message_port)) =
                        bincode::deserialize_from(&control_socket)
                            .map_err(|err| warn!("Handshake packet receive: {}", err))?;

                    let thread_message_sender = thread_message_sender.clone();
                    let connection_socket = display_err!(create_connection_socket(
                        control_socket,
                        address.ip(),
                        message_port,
                        message_receive_callback.clone(),
                        move || {
                            thread_message_sender.send(ThreadMessage::Data(())).ok();
                        },
                    ))?;

                    display_err!(connection_callback(
                        connection_socket,
                        server_handshake_packet
                    ))
                };

                loop {
                    if try_handshake().is_ok() {
                        // wait for disconnect or shutdown
                        if let Ok(ThreadMessage::Shutdown) = thread_message_receiver.recv() {
                            break;
                        }
                    }
                }

                Ok(())
            })
        };

        Self {
            thread_handles: Some(ThreadHandles {
                join_handle,
                message_sender: thread_message_sender,
            }),
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(ThreadHandles {
            join_handle,
            message_sender,
        }) = self.thread_handles.take()
        {
            message_sender.send(ThreadMessage::Shutdown).ok();
            join_handle.join().ok();
        }
    }
}

impl Drop for HandshakeSocket {
    fn drop(&mut self) {
        self.shutdown()
    }
}
