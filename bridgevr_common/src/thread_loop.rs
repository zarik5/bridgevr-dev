use std::sync::{Arc, atomic::*};
use std::thread::{self, JoinHandle};

pub struct ThreadLoop {
    join_handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl ThreadLoop {
    // this method is non blocking. Join will be called when dropped
    pub fn request_stop(&self) {
        self.running.store(false, Ordering::Relaxed)
    }
}

impl Drop for ThreadLoop {
    fn drop(&mut self) {
        self.request_stop();
        self.join_handle.take().map(|h| h.join());
    }
}

pub fn spawn(mut loop_body: impl FnMut() + Send + 'static) -> ThreadLoop {
    let running = Arc::new(AtomicBool::new(true));

    let join_handle = Some(thread::spawn({
        let running = running.clone();
        move || {
            while running.load(Ordering::Relaxed) {
                loop_body()
            }
        }
    }));

    ThreadLoop {
        join_handle,
        running,
    }
}
