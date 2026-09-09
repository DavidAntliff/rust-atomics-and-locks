use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::thread;

pub struct Channel<T> {
    queue: Mutex<VecDeque<T>>,
    item_ready: Condvar,
}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            item_ready: Condvar::new(),
        }
    }

    pub fn send(&self, message: T) {
        self.queue.lock().unwrap().push_back(message);
        self.item_ready.notify_one();
    }

    pub fn receive(&self) -> T {
        let mut b = self.queue.lock().unwrap();
        loop {
            if let Some(message) = b.pop_front() {
                return message;
            }
            b = self.item_ready.wait(b).unwrap();
        }
    }
}

pub fn demo() {
    let channel = Channel::<i32>::new();

    thread::scope(|s| {
        // producer
        s.spawn(|| {
            for i in 0..10 {
                channel.send(i);
            }
        });

        // consumer
        s.spawn(|| {
            for _ in 0..10 {
                let message = channel.receive();
                println!("Received: {}", message);
            }
        });
    });

    println!("simple_mutex_channel demo complete");
}
