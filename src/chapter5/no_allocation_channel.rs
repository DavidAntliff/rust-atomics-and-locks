//! Use the type system to statically guarantee that send is called at most once.
//! Allow the user to provide the Channel to avoid heap allocation via Arc.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
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
        (Sender { channel: self }, Receiver { channel: self })
    }
}


impl<T> Sender<'_, T> {
    pub fn send(self, message: T) {  // consumes self
        // Safety: this function consumes self, so can only be called once, therefore
        // no other thread is reading or writing the message while we initialize it.
        unsafe { (*self.channel.message.get()).write(message) };
        self.channel.ready.store(true, Release);
    }
}

impl<T> Receiver<'_, T> {
    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Relaxed)
    }

    pub fn receive(self) -> T {  // consumes self
        if !self.channel.ready.swap(false, Acquire) {
            panic!("no message available!");
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
        let t = thread::current();
        s.spawn(move || {
            sender.send("Hello, world!!");
            t.unpark();
        });
        while !receiver.is_ready() {
            thread::park();
        }
        let message = receiver.receive();
        println!("Received: {}", message);
        assert_eq!(message, "Hello, world!!");
    });
    println!("no_allocation_channel demo complete");
}
