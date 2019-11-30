use crate::timeout_map::TimeoutMap;
use std::sync::mpsc::{self, *};
use std::time::*;

#[derive(Debug)]
pub enum RingBufError {
    Timeout,
    Shutdown,
}

fn recv_to_ring_buffer_err(err: RecvTimeoutError) -> RingBufError {
    match err {
        RecvTimeoutError::Timeout => RingBufError::Timeout,
        RecvTimeoutError::Disconnected => RingBufError::Shutdown,
    }
}

pub struct Producer<I, O> {
    receiver: Receiver<I>,
    sender: Sender<O>,
    other_sender: Sender<I>,
}

impl<I, O> Producer<I, O> {
    pub fn add(&self, empty_buffer: I) {
        // this never panics because Producer owns both sender and receiver
        self.other_sender.send(empty_buffer).unwrap();
    }
}

impl<T> Producer<T, T> {
    pub fn fill(
        &self,
        timeout: Duration,
        callback: impl FnOnce(&mut T),
    ) -> Result<(), RingBufError> {
        let mut t = self
            .receiver
            .recv_timeout(timeout)
            .map_err(recv_to_ring_buffer_err)?;
        callback(&mut t);
        self.sender.send(t).map_err(|_| RingBufError::Shutdown)
    }
}

impl<K, V> Producer<V, (K, V)> {
    pub fn fill(
        &self,
        timeout: Duration,
        callback: impl FnOnce(&mut V) -> K,
    ) -> Result<(), RingBufError> {
        let mut v = self
            .receiver
            .recv_timeout(timeout)
            .map_err(recv_to_ring_buffer_err)?;
        let k = callback(&mut v);
        self.sender.send((k, v)).map_err(|_| RingBufError::Shutdown)
    }
}

pub struct Consumer<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> Consumer<T> {
    pub fn consume(
        &self,
        timeout: Duration,
        callback: impl FnOnce(&T),
    ) -> Result<(), RingBufError> {
        let t = self
            .receiver
            .recv_timeout(timeout)
            .map_err(recv_to_ring_buffer_err)?;
        callback(&t);
        self.sender.send(t).map_err(|_| RingBufError::Shutdown)
    }
}

pub struct KeyedConsumer<K, V> {
    sender: Sender<V>,
    receiver: Receiver<(K, V)>,
    map: TimeoutMap<K, V>,
}

impl<K, V> KeyedConsumer<K, V> {
    pub fn consume_any(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(K, &V),
    ) -> Result<(), RingBufError> {
        let (key, value) = if let Some(kv) = self.map.remove_any() {
            kv
        } else {
            self.receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?
        };
        callback(key, &value);
        for value in self.map.remove_expired() {
            self.sender
                .send(value)
                .map_err(|_| RingBufError::Shutdown)?;
        }
        self.sender.send(value).map_err(|_| RingBufError::Shutdown)
    }
}

impl<K: PartialEq, V> KeyedConsumer<K, V> {
    pub fn consume(
        &mut self,
        key: &K,
        timeout: Duration,
        callback: impl FnOnce(&V),
    ) -> Result<(), RingBufError> {
        let deadline = Instant::now() + timeout;
        let value = if let Some(value) = self.map.remove(key) {
            value
        } else {
            let (mut k, mut v) = self
                .receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?;
            while k != *key || Instant::now() < deadline {
                self.map.insert(k, v);
                let (new_k, new_v) = self
                    .receiver
                    .recv_timeout(deadline - Instant::now())
                    .map_err(recv_to_ring_buffer_err)?;
                k = new_k;
                v = new_v;
            }
            v
        };

        callback(&value);
        for value in self.map.remove_expired() {
            self.sender
                .send(value)
                .map_err(|_| RingBufError::Shutdown)?;
        }
        self.sender.send(value).map_err(|_| RingBufError::Shutdown)
    }
}

pub fn ring_buffer_split<T>() -> (Producer<T, T>, Consumer<T>) {
    let (producer_sender, consumer_receiver) = mpsc::channel();
    let (consumer_sender, producer_receiver) = mpsc::channel();
    let producer = Producer {
        receiver: producer_receiver,
        sender: producer_sender,
        other_sender: consumer_sender.clone(),
    };
    let consumer = Consumer {
        receiver: consumer_receiver,
        sender: consumer_sender,
    };

    (producer, consumer)
}

pub fn ring_buffer_split_keyed<K, V>(
    buffer_life: Duration,
) -> (Producer<V, (K, V)>, KeyedConsumer<K, V>) {
    let (producer_sender, consumer_receiver) = mpsc::channel();
    let (consumer_sender, producer_receiver) = mpsc::channel();
    let producer = Producer {
        receiver: producer_receiver,
        sender: producer_sender,
        other_sender: consumer_sender.clone(),
    };
    let consumer = KeyedConsumer {
        receiver: consumer_receiver,
        sender: consumer_sender,
        map: TimeoutMap::new(buffer_life),
    };

    (producer, consumer)
}
