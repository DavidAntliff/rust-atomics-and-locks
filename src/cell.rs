/// Lesson: T: Sync means exactly &T: Send.
///   - Send — can ownership cross a thread boundary? (a transfer; one thread has it at a time)
///   - Sync — can a reference be handed to several threads at once? (simultaneous access)
/// 
/// Cell is Send, but not Sync, and can't fail,
/// RefCell is Send (when T: Send) but not Sync, and panics,
/// Mutex is Sync, and blocks, and can deadlock,
/// Atomic types are Send and Sync, and can't fail, but are limited to Copy types.
/// 
use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

/// `Cell<T>: Send` when `T: Send`, but never `Sync`. So a struct holding one
/// can *move* to another thread; it just can't be *shared* with one.
#[derive(Debug)]
struct Counter {
    hits: Cell<u32>,
}

impl Counter {
    fn hit(&self) {
        // interior mutability: &self, not &mut self
        self.hits.set(self.hits.get() + 1);
    }
}

pub fn cell_to_thread() {
    let c = Counter { hits: Cell::new(0) };
    c.hit();

    let h = thread::spawn(move || {
        c.hit();
        c.hit();
        c // move it back out — the only way to see it again
    });

    let c = h.join().unwrap();
    println!("{c:?}");
    assert_eq!(c.hits.get(), 3);
}

/// `AtomicU32` is `Sync`, so `&CounterSync` is `Send` and the struct can be
/// *shared*. No move, no `Arc`, no handing it back out.
#[derive(Debug)]
struct CounterSync {
    hits: AtomicU32,
}

impl CounterSync {
    fn hit(&self) {
        self.hits.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn atomic_to_thread() {
    let c = CounterSync { hits: AtomicU32::new(0) };
    c.hit();

    thread::scope(|s| {
        s.spawn(|| c.hit()); // both closures capture &c
        s.spawn(|| c.hit()); // two threads, one counter, genuinely shared
    });

    println!("{c:?}");
    assert_eq!(c.hits.load(Ordering::SeqCst), 3);
}
