// MaybeUninit is like a bare-bones Option (uses less memory)

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    in_use: AtomicBool,
    ready: AtomicBool,
}

// Tell the compiler that our channel is safe to share between threads, provided that T is Send.
unsafe impl<T> Sync for Channel<T>
where
    T: Send,
{}

impl<T> Channel<T> {
    /// A new channel is empty.
    pub const fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            in_use: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }

    /// Panics when trying to send more than one message.
    pub fn send(&self, message: T) {
        if self.in_use.swap(true, Relaxed) {
            panic!("can't send more than one message!");
        }
        // Safety: the caller guarantees this is called only once, so no other
        // thread is reading or writing the message while we initialize it.
        unsafe { (*self.message.get()).write(message) };
        self.ready.store(true, Release);
    }

    pub fn is_ready(&self) -> bool {
        // Relaxed is sufficient here because we use Acquire in `receive()`.
        self.ready.load(Relaxed)
    }

    /// Panics if no message is available yet, or if the message was already consumed.
    /// Use `is_ready()` to check first.
    pub fn receive(&self) -> T {
        if !self.ready.swap(false, Acquire) {
            panic!("no message available!");
        }
        // Safety: we've just checked (and reset) the ready flag, so we know the message
        // is initialized and hasn't been consumed yet.
        unsafe { (*self.message.get()).assume_init_read() }
    }

    // Note lack of Drop implementation - leaks memory if the channel is dropped when inhabited.
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            // Safety: an object can only be dropped once, if fully owned.
            unsafe { self.message.get_mut().assume_init_drop() }
        }
    }
}

pub fn demo() {
    let channel = Channel::new();
    let t = thread::current();
    thread::scope(|s| {
        s.spawn(|| {
            channel.send("Hello, world!!");
            // should panic:
            //channel.send("Hello, world!!");
            t.unpark();
        });
        while !channel.is_ready() {
            thread::park();
        }
        let message = channel.receive();
        println!("Received: {}", message);
        assert_eq!(message, "Hello, world!!");
    });
    println!("one_shot_channel_checked demo complete");
}
