use std::collections::VecDeque;
use std::sync::Arc;
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

/// `done` lives *inside* the mutex, so the producer can't set it in the window
/// between the consumer's check and its `wait` — no lost wakeup.
#[derive(Default)]
struct Queue {
    items: VecDeque<i32>,
    done: bool,
}

pub fn condition_var() {
    let state = Mutex::new(Queue::default());
    // One condvar: "the state changed, re-check your predicate."
    let ready = Condvar::new();

    thread::scope(|s| {
        s.spawn(|| {
            loop {
                // Wait until there's an item OR we're done — one wait, two conditions.
                let mut q = ready
                    .wait_while(state.lock().unwrap(), |q| q.items.is_empty() && !q.done)
                    .unwrap();
                match q.items.pop_front() {
                    Some(item) => {
                        drop(q); // don't hold the lock while working
                        dbg!(item);
                    }
                    None => break, // empty *and* done
                }
            }
        });

        for i in 0..20 {
            state.lock().unwrap().items.push_back(i);
            ready.notify_one();
            thread::sleep(Duration::from_millis(50));
        }

        state.lock().unwrap().done = true;
        ready.notify_one(); // wake it up to finish
    });
}

/// A panic while holding the lock *poisons* the mutex: every later `lock()`
/// returns `Err`. That `Err` still carries the guard, so recovery is possible —
/// it's a warning that the data may be half-updated, not a lockout.
pub fn poisoning() {
    // Strategy 3 from chapter 1: shared ownership, since neither thread
    // outlives the other in a way `scope` could prove.
    let data = Arc::new(Mutex::new(vec![1, 2, 3]));

    let d = Arc::clone(&data);
    let handle = thread::spawn(move || {
        let mut v = d.lock().unwrap();
        v.push(4);
        panic!("boom"); // unwinding drops the guard -> mutex is poisoned
    });

    // The panic is delivered here, as an Err from join.
    assert!(handle.join().is_err());
    assert!(data.is_poisoned());

    // The mutex is unlocked, but locking it still fails.
    assert!(data.lock().is_err());

    // Recover deliberately: the guard is inside the error.
    let v = data.lock().unwrap_or_else(|e| e.into_inner());
    println!("recovered after poisoning: {v:?}");
    assert_eq!(*v, vec![1, 2, 3, 4]); // the half-finished update is visible
    drop(v);

    // Declare the data sane again; lock() succeeds from here on.
    data.clear_poison();
    assert!(data.lock().is_ok());
}

pub fn arc_sharing() {
    let data = vec![1, 2, 3];
    let data = Arc::new(data); // Arc is Send + Sync when T is, so it can be shared

    assert_eq!(Arc::strong_count(&data), 1);

    let t1 = thread::spawn({
        let data = Arc::clone(&data);
        move || {
            assert!(Arc::strong_count(&data) >= 2);
            println!("{data:?}");
        }
    });

    let t2 = thread::spawn({
        let data = Arc::clone(&data);
        move || {
            assert!(Arc::strong_count(&data) >= 2);
            println!("{data:?}");
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();

    assert_eq!(Arc::strong_count(&data), 1);
}
