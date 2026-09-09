//! Use the type system to statically guarantee that send is called at most once.
//! Block callers properly, but only the thread that calls split() may call receive().
//! Allow the user to provide the Channel to avoid heap allocation via Arc.

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;
use std::thread::Thread;

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
    receiving_thread: Thread, // the thread to call unpack() on
}

// Prevent the thread from being Send, so that receiving_thread is always correct.
pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
    _no_send: std::marker::PhantomData<*const ()>, // raw pointer does not implement Send
}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
        }
    }

    pub fn split<'a>(&'a mut self) -> (Sender<'a, T>, Receiver<'a, T>) {
        *self = Self::new(); // fresh start, ensure invariants hold if called twice
        (
            Sender {
                channel: self,
                receiving_thread: thread::current(),
            },
            Receiver {
                channel: self,
                _no_send: PhantomData,
            },
        )
    }
}

impl<T> Sender<'_, T> {
    pub fn send(self, message: T) {
        // consumes self
        // Safety: this function consumes self, so can only be called once, therefore
        // no other thread is reading or writing the message while we initialize it.
        unsafe { (*self.channel.message.get()).write(message) };
        self.channel.ready.store(true, Release);
        self.receiving_thread.unpark();
    }
}

impl<T> Receiver<'_, T> {
    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Relaxed)
    }

    pub fn receive(self) -> T {
        // consumes self

        // Instead of panicking, wait for a message using thread::park()
        while !self.channel.ready.swap(false, Acquire) {
            thread::park();
        }
        // Safety: we've just checked (and reset) the ready flag, so we know the message
        // is initialized and hasn't been consumed yet.
        unsafe { (*self.channel.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            // Safety: we know the message is initialized, because ready is true.
            unsafe { (*self.message.get()).assume_init_drop() };
        }
    }
}

pub fn demo() {
    // Must be created outside the scope, so that it is guaranteed
    // to outlast the sender and receiver:
    let mut channel = Channel::new();

    thread::scope(|s| {
        let (sender, receiver) = channel.split();
        s.spawn(move || {
            sender.send("Hello, world!!");
        });

        let message = receiver.receive();
        println!("Received: {}", message);
        assert_eq!(message, "Hello, world!!");
    });
    println!("blocking_channel demo complete");
}
