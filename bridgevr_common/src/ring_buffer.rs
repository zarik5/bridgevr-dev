use crate::timeout_map::TimeoutMap;
use std::collections::VecDeque;
use std::sync::mpsc::{self, *};
use std::time::*;

pub type UnitResult = Result<(), ()>;

#[derive(Debug)]
pub enum RingBufError {
    Timeout,
    Shutdown,
}

type RingBufResult = Result<(), RingBufError>;

fn recv_to_ring_buffer_err(err: RecvTimeoutError) -> RingBufError {
    match err {
        RecvTimeoutError::Timeout => RingBufError::Timeout,
        RecvTimeoutError::Disconnected => RingBufError::Shutdown,
    }
}

// Abstraction layer over VecDeque and TimedMap
pub trait Collection<K, V> {
    fn push_back(&mut self, key: K, value: V);
    fn push_front(&mut self, key: K, value: V);
    fn remove(&mut self, key: &K) -> Option<(K, V)>;
    fn remove_any(&mut self) -> Option<(K, V)>;
    fn remove_expired(&mut self) -> Vec<V>;
}

impl<T> Collection<(), T> for VecDeque<T> {
    fn push_back(&mut self, _: (), value: T) {
        VecDeque::push_back(self, value);
    }
    fn push_front(&mut self, _: (), value: T) {
        VecDeque::push_front(self, value);
    }
    fn remove(&mut self, _: &()) -> Option<((), T)> {
        self.pop_front().map(|v| ((), v))
    }
    fn remove_any(&mut self) -> Option<((), T)> {
        self.pop_front().map(|v| ((), v))
    }
    fn remove_expired(&mut self) -> Vec<T> {
        vec![]
    }
}

impl<K: PartialEq, V> Collection<K, V> for TimeoutMap<K, V> {
    fn push_back(&mut self, key: K, value: V) {
        self.insert(key, value);
    }
    fn push_front(&mut self, key: K, value: V) {
        self.insert(key, value);
    }
    fn remove(&mut self, key: &K) -> Option<(K, V)> {
        TimeoutMap::remove(self, key)
    }
    fn remove_any(&mut self) -> Option<(K, V)> {
        TimeoutMap::remove_any(self)
    }
    fn remove_expired(&mut self) -> Vec<V> {
        TimeoutMap::remove_expired(self)
    }
}

// If not explicitely calling add(), a Producer does not actually create Vs but only modify them.
pub struct Producer<V, K = ()> {
    sender: Sender<(K, V)>,
    receiver: Receiver<V>,
    queue: VecDeque<V>,
}

impl<K, V> Producer<V, K> {
    pub fn add(&mut self, empty: V) {
        self.queue.push_back(empty);
    }

    pub fn wait_for_empty(&mut self, timeout: Duration) -> RingBufResult {
        let value = self.receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?;
        self.queue.push_back(value);
        Ok(())
    }

    pub fn fill(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&mut V) -> Result<K, ()>,
    ) -> RingBufResult {
        let mut value = if let Some(v) = self.queue.pop_front() {
            v
        } else {
            self.receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?
        };
        if let Ok(key) = callback(&mut value) {
            self.sender
                .send((key, value))
                .map_err(|_| RingBufError::Shutdown)?;
        } else {
            self.queue.push_front(value);
        }
        Ok(())
    }
}

pub struct Consumer<V, K = (), C = VecDeque<V>> {
    sender: Sender<V>,
    receiver: Receiver<(K, V)>,
    buffer: C,
}

impl<K, V, C: Collection<K, V>> Consumer<V, K, C> {
    pub fn push(&self, empty: V) -> RingBufResult {
        self.sender.send(empty).map_err(|_| RingBufError::Shutdown)
    }

    fn consume_element(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&K, &V) -> UnitResult,
    ) -> RingBufResult {
        let (k, v) = if let Some(kv) = self.buffer.remove_any() {
            kv
        } else {
            self.receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?
        };
        if callback(&k, &v).is_ok() {
            self.sender.send(v).map_err(|_| RingBufError::Shutdown)?;
        } else {
            self.buffer.push_front(k, v);
        }
        for value in self.buffer.remove_expired() {
            self.sender
                .send(value)
                .map_err(|_| RingBufError::Shutdown)?;
        }
        Ok(())
    }
}
impl<K: PartialEq, V, C: Collection<K, V>> Consumer<V, K, C> {
    fn consume_entry(
        &mut self,
        key: &K,
        timeout: Duration,
        callback: impl FnOnce(&V) -> UnitResult,
    ) -> RingBufResult {
        let deadline = Instant::now() + timeout;
        let (k, v) = if let Some(kv) = self.buffer.remove(key) {
            kv
        } else {
            let mut kv = self
                .receiver
                .recv_timeout(timeout)
                .map_err(recv_to_ring_buffer_err)?;
            while kv.0 != *key || Instant::now() < deadline {
                self.buffer.push_back(kv.0, kv.1);
                kv = self
                    .receiver
                    .recv_timeout(deadline - Instant::now())
                    .map_err(recv_to_ring_buffer_err)?;
            }
            kv
        };

        if callback(&v).is_ok() {
            self.sender.send(v).map_err(|_| RingBufError::Shutdown)?;
        } else {
            self.buffer.push_front(k, v);
        }
        for value in self.buffer.remove_expired() {
            self.sender
                .send(value)
                .map_err(|_| RingBufError::Shutdown)?;
        }
        Ok(())
    }
}

impl<T> Consumer<T, (), VecDeque<T>> {
    pub fn consume(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&T) -> UnitResult,
    ) -> RingBufResult {
        self.consume_element(timeout, |_, t| callback(t))
    }
}

impl<K: PartialEq, V> Consumer<V, K, TimeoutMap<K, V>> {
    pub fn consume_any(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&K, &V) -> UnitResult,
    ) -> RingBufResult {
        self.consume_element(timeout, callback)
    }

    pub fn consume(
        &mut self,
        key: &K,
        timeout: Duration,
        callback: impl FnOnce(&V) -> UnitResult,
    ) -> RingBufResult {
        self.consume_entry(key, timeout, callback)
    }
}

pub type KeyedConsumer<V, K> = Consumer<V, K, TimeoutMap<K, V>>;

fn ring_buffer_split<V, K, C>(consumer_buffer: C) -> (Producer<V, K>, Consumer<V, K, C>) {
    let (producer_sender, consumer_receiver) = mpsc::channel();
    let (consumer_sender, producer_receiver) = mpsc::channel();
    let producer = Producer {
        receiver: producer_receiver,
        sender: producer_sender,
        queue: VecDeque::new(),
    };
    let consumer = Consumer {
        receiver: consumer_receiver,
        sender: consumer_sender,
        buffer: consumer_buffer,
    };

    (producer, consumer)
}

pub fn queue_ring_buffer_split<T>() -> (Producer<T>, Consumer<T>) {
    ring_buffer_split(VecDeque::new())
}

pub fn keyed_ring_buffer_split<K, V>(
    values_timeout: Duration,
) -> (Producer<V, K>, KeyedConsumer<V, K>) {
    ring_buffer_split(TimeoutMap::new(values_timeout))
}