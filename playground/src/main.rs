use std::sync::*;

fn main() {
    let mutex = Arc::new(Mutex::new(()));

    // let maybe_guard = Arc::new(Mutex::new(None));

    let fdfsdf = {
        let mutex_clone = mutex.clone();
        // let maybe_guard_clone = maybe_guard.clone();
        move || Some(mutex_clone.lock())
    };
}
