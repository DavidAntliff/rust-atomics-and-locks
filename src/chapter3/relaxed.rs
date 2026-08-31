use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::thread;

pub fn happens_before() {
    let wait = &AtomicBool::new(true);

    static X: AtomicI32 = AtomicI32::new(0);
    static Y: AtomicI32 = AtomicI32::new(0);

    fn a() {
        X.store(10, Relaxed);
        Y.store(20, Relaxed);
    }

    fn b() {
        let y = Y.load(Relaxed);
        let x = X.load(Relaxed);
        match (x, y) {
            (0, 0) => (), //println!("Both are zero"),
            (10, 20) => (), //println!("Both are set"),
            _ => println!("Unexpected values: x = {x}, y = {y}"),
        }
    }

    thread::scope(|s| {
        for x in 0..1000 {
            s.spawn(|| {
                while wait.load(Relaxed) {
                    thread::yield_now();
                }
                a()
            });
            s.spawn(|| {
                while wait.load(Relaxed) {
                    thread::yield_now();
                }
                b()
            });
        }

        thread::sleep(std::time::Duration::from_secs(1));
        wait.store(false, Relaxed);
    });
}
