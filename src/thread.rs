use std::collections::VecDeque;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub fn parking() {
    let queue = Mutex::new(VecDeque::new());
    let done = AtomicBool::new(false);

    thread::scope(|s| {
        // Consuming thread
        let t = s.spawn(|| loop {
            let item = queue.lock().unwrap().pop_front();
            if let Some(item) = item {
                dbg!(item);
            } else if done.load(Ordering::Relaxed) {
                break;
            } else {
                thread::park();
            }
        });

        // Producing thread
        for i in 0..20 {
            queue.lock().unwrap().push_back(i);
            t.thread().unpark();
            thread::sleep(Duration::from_millis(50));
        }

        done.store(true, Ordering::Relaxed);
        t.thread().unpark(); // wake it up to finish
    });
}

pub fn condition_var() {
    let queue = Mutex::new(VecDeque::new());
    let done = AtomicBool::new(false);
    let not_empty = Condvar::new();

    thread::scope(|s| {
        s.spawn(|| {
            loop {
                let mut q = queue.lock().unwrap();
                let item = loop {
                    if let Some(item) = q.pop_front() {
                        break Some(item);
                    } else if done.load(Ordering::Relaxed) {
                        break None;
                    } else {
                        q = not_empty.wait(q).unwrap();
                    }
                };
                if let Some(item) = item {
                    drop(q);
                    dbg!(item);
                } else {
                    break;
                }
            }
        });

        for i in 0..20 {
            queue.lock().unwrap().push_back(i);
            not_empty.notify_one();
            thread::sleep(Duration::from_millis(50));
        }

        done.store(true, Ordering::Relaxed);
        not_empty.notify_one(); // wake it up to finish
    });
}
