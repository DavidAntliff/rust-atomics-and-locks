use std::sync::Barrier;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::thread;

/// Initialise a complex value lazily using atomic pointer indirection
pub fn lazy_init_with_indirection() {
    //let wait = &AtomicBool::new(true);
    const WORKERS: usize = 10;
    let ready = Barrier::new(WORKERS + 1);  // workers + main
    let go = AtomicBool::new(false);
    let (ready, go) = (&ready, &go);

    thread::scope(|s| {
        // All threads should get the same value
        for t in 0..WORKERS {
            s.spawn(move || {
                // while wait.load(Relaxed) {
                //     thread::yield_now();
                // }
                ready.wait();
                while !go.load(Relaxed) {
                    std::hint::spin_loop();  // no syscall!
                }

                let data = get_data(t);
                println!("{t} Data: ({}, {}, {}, {})", data.0, data.1, data.2, data.3);
            });
        }

        ready.wait();  // the last call unblocks all threads
        thread::sleep(std::time::Duration::from_millis(1));
        //thread::sleep(std::time::Duration::from_millis(100));
        //wait.store(false, Relaxed);
        go.store(true, Relaxed);
    });

    let data = get_data(0);
    println!("Final Data: ({}, {}, {}, {})", data.0, data.1, data.2, data.3);
}

struct Data(usize, usize, usize, usize);

impl Data {
    fn new(t: usize) -> Self {
        Data(t + 1, t + 2, t + 3, t + 4)
    }
}

fn get_data(t: usize) -> &'static Data {
    static PTR: AtomicPtr<Data> = AtomicPtr::new(std::ptr::null_mut());

    // Using acquire establishes a happens-before relationship with the release store
    let mut p = PTR.load(Ordering::Acquire);

    if p.is_null() {
        p = Box::into_raw(Box::new(Data::new(t)));
        if let Err(e) = PTR.compare_exchange(std::ptr::null_mut(), p, Ordering::Release, Ordering::Acquire) {
            // Another thread beat us to it, so we need to free the memory we just allocated
            // Safety: p comes from Box::into_raw right above,
            // and wasn't shared with any other thread:
            drop(unsafe { Box::from_raw(p) });
            p = e;
        }
    }

    // Safety: p is not null and points to a properly initialised Data value
    unsafe { &*p }
}
