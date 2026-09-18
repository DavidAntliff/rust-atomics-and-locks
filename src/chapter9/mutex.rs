use atomic_wait::{wait, wake_one};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::{Acquire, Release};

pub struct Mutex<T> {
    /// 0: unlocked, 1: locked
    state: AtomicU32,
    value: UnsafeCell<T>,
}

// Ensure that Mutex<T> is Sync if T is Send, so that we can safely share a Mutex<T> between threads
unsafe impl<T> Sync for Mutex<T>
where
    T: Send,
{}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),  // unlocked
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        // Set the state to 1: locked
        while self.state.swap(1, Acquire) == 1 {
            // If it was already locked, wait, unless the state is no longer 1
            wait(&self.state, 1);
        }
        MutexGuard { mutex: self }
    }
}


pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

unsafe impl<T> Send for MutexGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for MutexGuard<'_, T> where T: Sync {}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // Set the state back to 0: unlocked
        self.mutex.state.store(0, Release);
        // Wake up one of the waiting threads, if any
        wake_one(&self.mutex.state);
    }
}

// Important note - wait() and wake_one() are not necessary for correctness,
// but they are important for performance. Without them, this is an inefficient spinlock, but it
// will still work correctly.
