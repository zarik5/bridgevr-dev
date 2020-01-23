// Utility used in place of std::sync::mpsc for managing buffers (reusable memory owned by the
// sender)

// todo: since this is not used by sockets anymore, re-convert to a spsc

use parking_lot::*;
use std::{
    collections::*,
    ops::RangeFrom,
    sync::{
        mpsc::{self, *},
        Arc,
    },
    time::*,
};

#[derive(Debug)]
pub enum BufChanError<E> {
    Timeout,
    Disconnected,
    Callback(E),
}

fn recv_to_buf_chan_err<E>(err: RecvTimeoutError) -> BufChanError<E> {
    match err {
        RecvTimeoutError::Timeout => BufChanError::Timeout,
        RecvTimeoutError::Disconnected => BufChanError::Disconnected,
    }
}

// The approach of using InternalMessage to setup new Senders adds one more check per received
// buffer by the Receiver, this is better than using a mutex to share the return_senders.
enum InternalMessage<T> {
    FilledBuffer(usize, T),
    NewSender(usize, mpsc::Sender<T>),
}

pub struct Sender<T> {
    // id is used by the Receiver to identify the Sender to which to return the read buffer
    id: usize,
    id_iterator: Arc<Mutex<RangeFrom<usize>>>,
    message_sender: mpsc::Sender<InternalMessage<T>>,
    return_receiver: mpsc::Receiver<T>,
    queue: VecDeque<T>,
}

impl<T> Sender<T> {
    pub fn add_empty_buffer(&mut self, empty: T) {
        self.queue.push_back(empty);
    }

    pub fn wait_for_some_buffers(&mut self, timeout: Duration) -> Result<(), RecvTimeoutError> {
        if self.queue.is_empty() {
            let buffer = self.return_receiver.recv_timeout(timeout)?;
            self.queue.push_back(buffer);
        }
        Ok(())
    }

    // callback return: true if buffer is filled, false otherwise
    pub fn fill<E>(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&mut T) -> Result<bool, E>,
    ) -> Result<(), BufChanError<E>> {
        let mut buffer = if let Some(buffer) = self.queue.pop_front() {
            buffer
        } else {
            self.return_receiver
                .recv_timeout(timeout)
                .map_err(recv_to_buf_chan_err)?
        };

        match callback(&mut buffer) {
            Ok(true) => {
                if let Err(SendError(InternalMessage::FilledBuffer(_, buffer))) = self
                    .message_sender
                    .send(InternalMessage::FilledBuffer(self.id, buffer))
                {
                    self.queue.push_front(buffer);
                    Err(BufChanError::Disconnected)
                } else {
                    Ok(())
                }
            }
            Ok(false) => {
                self.queue.push_front(buffer);
                Ok(())
            }
            Err(user_err) => {
                self.queue.push_front(buffer);
                Err(BufChanError::Callback(user_err))
            }
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let id = self.id_iterator.lock().next().unwrap();
        let (return_sender, return_receiver) = mpsc::channel();

        // If this fails, an error will be detected when using the new Sender
        self.message_sender
            .send(InternalMessage::NewSender(id, return_sender))
            .ok();

        Self {
            id,
            id_iterator: self.id_iterator.clone(),
            message_sender: self.message_sender.clone(),
            return_receiver,
            queue: VecDeque::new(),
        }
    }
}

pub struct Receiver<T> {
    message_receiver: mpsc::Receiver<InternalMessage<T>>,
    return_senders: HashMap<usize, mpsc::Sender<T>>,
    queue: VecDeque<(usize, T)>,
}

impl<T> Receiver<T> {
    // callback return: true if buffer is read, false otherwise
    pub fn consume<E>(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&T) -> Result<bool, E>,
    ) -> Result<(), BufChanError<E>> {
        let (id, buffer) = if let Some(id_buffer) = self.queue.pop_front() {
            id_buffer
        } else {
            loop {
                match self.message_receiver.recv_timeout(timeout) {
                    Ok(InternalMessage::FilledBuffer(id, buffer)) => break (id, buffer),
                    Ok(InternalMessage::NewSender(id, return_sender)) => {
                        self.return_senders.insert(id, return_sender);
                        continue;
                    }
                    Err(recv_err) => return Err(recv_to_buf_chan_err(recv_err)),
                }
            }
        };

        match callback(&buffer) {
            Ok(true) => {
                // this unwrap should never fail
                let sender = self.return_senders.get(&id).unwrap();
                if let Err(SendError(buffer)) = sender.send(buffer) {
                    self.queue.push_front((id, buffer));
                    Err(BufChanError::Disconnected)
                } else {
                    Ok(())
                }
            }
            Ok(false) => {
                self.queue.push_front((id, buffer));
                Ok(())
            }
            Err(user_err) => {
                self.queue.push_front((id, buffer));
                Err(BufChanError::Callback(user_err))
            }
        }
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let mut id_iterator = 0..;
    // this unwrap cannot fail
    let id = id_iterator.next().unwrap();

    let (message_sender, message_receiver) = mpsc::channel();
    let (return_sender, return_receiver) = mpsc::channel();
    let sender = Sender {
        // this unwrap cannot fail
        id,
        id_iterator: Arc::new(Mutex::new(id_iterator)),
        message_sender,
        return_receiver,
        queue: VecDeque::new(),
    };

    let receiver = Receiver {
        message_receiver,
        return_senders: vec![(id, return_sender)].into_iter().collect(),
        queue: VecDeque::new(),
    };

    (sender, receiver)
}
