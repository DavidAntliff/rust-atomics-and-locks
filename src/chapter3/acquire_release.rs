use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::thread;
use std::time::Duration;

pub fn acquire_release() {
    static mut DATA: u64 = 0;
    static READY: AtomicBool = AtomicBool::new(false);

    thread::scope(|s| {
        s.spawn(|| {
            // Safety: Nothing else is accessing DATA, because we haven't set the READY flag yet.
            unsafe { DATA = 42; };
            READY.store(true, Release);  // Everything from before this store...
        });

        while !READY.load(Acquire) {  // ... is visible after this loads 'true'.
            thread::sleep(Duration::from_millis(100));
            println!("waiting...");
        }

        // Safety: Nothing is mutating DATA, because READY is set.
        println!("{}", unsafe { DATA });
    });
}
