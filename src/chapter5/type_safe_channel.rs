//! Use the type system to statically guarantee that send is called at most once.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;

pub struct Sender<T> {
    channel: Arc<Channel<T>>,
}

pub struct Receiver<T> {
    channel: Arc<Channel<T>>,
}

// Not pub
struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let a = Arc::new(Channel {
        message: UnsafeCell::new(MaybeUninit::uninit()),
        ready: AtomicBool::new(false),
    });
    (Sender { channel: a.clone() }, Receiver { channel: a })
}

impl<T> Sender<T> {
    /// This never panics
    pub fn send(self, message: T) {  // consumes self
        // Safety: this function consumes self, so can only be called once, therefore
        // no other thread is reading or writing the message while we initialize it.
        unsafe { (*self.channel.message.get()).write(message) };
        self.channel.ready.store(true, Release);
    }
}

impl<T> Receiver<T> {
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
    thread::scope(|s| {
        let (sender, receiver) = channel();
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
    println!("type_safe_channel demo complete");
}
