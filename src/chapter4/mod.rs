use std::thread;

mod spinlock;

pub fn run_all() {
    demo();
}

pub fn demo() {
    let x = spinlock::SpinLock::new(Vec::new());

    thread::scope(|s| {
        s.spawn(|| x.lock().push(1));
        s.spawn(|| {
            let mut g = x.lock();
            g.push(2);
            g.push(2);
        });
    });

    let g = x.lock();
    assert!(g.as_slice() == [1, 2, 2] || g.as_slice() == [2, 2, 1]);
}
