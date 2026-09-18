// The optimised version still makes syscalls in contended cases.
// It would be more efficient if it could avoid syscalls in the common case where a lock
// is held on a different CPU for a very short period of time.
// Spin for a short time before resorting to a syscall.
//
// The simple implementation makes a syscall for every lock and unlock.
// We can avoid this in the uncontended case by introducing two locked states,
// allowing us to avoid syscalls when there are no other waiters.
//
// Important note - wait() and wake_one() are not necessary for correctness,
// but they are important for performance. Without them, this is an inefficient spinlock, but it
// will still work correctly.

use atomic_wait::{wait, wake_one};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

const MAX_SPIN_COUNT: usize = 100;  // std::sync::Mutex uses 100

#[derive(Debug)]
pub struct Mutex<T> {
    /// 0: unlocked,
    /// 1: locked, no other threads waiting
    /// 2: locked, other threads waiting
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

    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Try setting the state from 0 to 1:
        if self.state.compare_exchange(0, 1, Acquire, Relaxed).is_err() {
            // The lock was already locked :(
            lock_contended(&self.state);
        }
        MutexGuard { mutex: self }
    }
}

fn lock_contended(state: &AtomicU32) {
    // compare-and-exchange tends to get exclusive cache-line access, so can
    // be expensive if run in a loop - use load instead.

    let mut spin_count = 0;

    while state.load(Relaxed) == 1 && spin_count < MAX_SPIN_COUNT {
        spin_count += 1;
        std::hint::spin_loop();
    }

    if state.compare_exchange(0, 1, Acquire, Relaxed).is_ok() {
        return; // we got the lock
    }

    // Failed, so must already be in state 1 or 2 - try setting to 2
    while state.swap(2, Acquire) != 0 {
        // If the old value was not 0, it was already locked, so we can wait
        wait(state, 2);
    } // else we changed it from 0 to 2
}

#[derive(Debug)]
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
        if self.mutex.state.swap(0, Release) == 2 {
            // There are other waiters
            wake_one(&self.mutex.state);
        }
    }
}
