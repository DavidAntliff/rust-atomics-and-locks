use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};

/// Responsible for the lock state until dropped.
/// The lifetime guarantees that the guard cannot outlive the lock it is guarding.
pub struct Guard<'lock, T> {
    /// No constructor and private field ensures that the
    /// only way to get a Guard is through the lock() method.
    lock: &'lock SpinLock<T>,
}

/// Make the guard behave like an exclusive reference to T
impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Safety: the very existence of this Guard guarantees we've exclusively locked the lock.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // Safety: the very existence of this Guard guarantees we've exclusively locked the lock.
        unsafe { &mut *self.lock.value.get() }
    }
}

/// Make sure our Guard is only Sync if T is Sync (and Send if T is Send), to avoid allowing
/// multiple threads to share a single Guard<T> and concurrently access the same T even when
/// T is not Sync.
/// If we leave it to the compiler, it will use the SpinLock, not T, to determine whether
/// Guard is Send and/or Sync, which is not what we want.
unsafe impl<T> Send for Guard<'_, T>
where
    T: Send,
{}
unsafe impl<T> Sync for Guard<'_, T> where T: Sync {}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Release);
    }
}


pub struct SpinLock<T> {
    /// Are we locked, or not?
    locked: AtomicBool,
    /// The value we are protecting
    value: UnsafeCell<T>,
}

// We need to promise that our type is safe to be shared between threads,
// provided that the value we are protecting is safe to be *sent* between threads.
// T does not need to be Sync, because we are not sharing it between threads,
// we are moving it in and out of the lock.
unsafe impl<T> Sync for SpinLock<T>
where
    T: Send,
{}

/// Acquire/Release memory ordering to ensure that every `unlock()` happens-before every `lock()`.
impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> Guard<T> {
        while self.locked.swap(true, Acquire) {
            std::hint::spin_loop();  // no syscall
        }
        // alternative impl:
        // while self.locked.compare_exchange_weak(
        //     false, true, Acquire, Relaxed).is_err() {
        //     std::hint::spin_loop();
        // }

        Guard { lock: self }
    }

    /// Safety: the &mut T from lock() must no longer exist!
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Release);
    }
}
