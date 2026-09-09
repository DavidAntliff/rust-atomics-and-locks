// MaybeUninit is like a bare-bones Option (uses less memory)

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
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
            ready: AtomicBool::new(false),
        }
    }

    /// Safety: only call this once!
    pub unsafe fn send(&self, message: T) {
        // Safety: the caller guarantees this is called only once, so no other
        // thread is reading or writing the message while we initialize it.
        unsafe { (*self.message.get()).write(message) };
        self.ready.store(true, Release);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Acquire)
    }

    /// Safety: only call this once, and only after is_ready() returns true!
    pub unsafe fn receive(&self) -> T {
        // Safety: assume_init_read requires that the MaybeUninit is initialized, which is true if
        // and only if is_ready() returned true.
        unsafe { (*self.message.get()).assume_init_read() }
    }

    // Note lack of Drop implementation - leaks memory if the channel is dropped when inhabited.
}
