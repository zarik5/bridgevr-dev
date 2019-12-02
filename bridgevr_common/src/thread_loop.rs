use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub struct ThreadLoop {
    join_handle: Option<JoinHandle<()>>,
    running: Arc<Mutex<bool>>,
}

impl ThreadLoop {
    // this method is non blocking. Join will be called when dropped
    pub fn request_stop(&self) {
        *self.running.lock().unwrap() = false;
    }
}

impl Drop for ThreadLoop {
    fn drop(&mut self) {
        self.request_stop();
        self.join_handle.take().map(|h| h.join());
    }
}

pub fn spawn(mut loop_body: impl FnMut() + Send + 'static) -> ThreadLoop {
    let running = Arc::new(Mutex::new(true));

    let join_handle = Some(thread::spawn({
        let running = running.clone();
        move || {
            while *running.lock().unwrap() {
                loop_body();
            }
        }
    }));

    ThreadLoop {
        join_handle,
        running,
    }
}
