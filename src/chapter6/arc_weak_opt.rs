//! Optimised Arc implementation with Weak pointers
//!
//! The prior implementation has a downside where cloning and dropping an Arc requires
//! two atomic operations each, as both counters have to be modified.
//! This makes payment for Weak pointers mandatory, even if they are not used.
//!
//! We can't count Arc and Weak separately, because we couldn't atomically check that both are zero.
//!
//! Instead, we can count every Arc combined as one single Weak pointer. Thus, only dropping the
//! very last Arc will decrement the Weak counter.
//!
//! Each Weak counts as 1,
//! All Arcs together count as just 1 (an implicit weak).
//!
//! So if there is at least one Arc, alloc_ref_count is at least 1.
//! When the last Arc is dropped, that implicit 1 is removed, and the payload data is dropped.
//! After that, only real Weaks keep the allocation memory alive.
//!
//! Thus, data_ref_count counts for data liveness, and alloc_ref_count counts for allocation liveness.
use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicUsize, fence};

// Use a pointer and handle allocation and ownership manually
pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

// Arc<T> should be Send iff T is both Send & Sync (sending to another thread might result in drop).
// Arc<T> should be Sync iff T is both Send & Sync (multiple threads can access the same Arc<T>).
unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

// An object responsible for keeping an ArcData<T> alive
pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}

// Internal details, which will be shared
// Make slightly smaller with ManuallyDrop instead of Option
struct ArcData<T> {
    /// Number of Arcs
    data_ref_count: AtomicUsize,
    /// Number of Weaks, plus one if there are any Arcs
    alloc_ref_count: AtomicUsize,
    /// The data. Dropped if there are only weak pointers left
    data: UnsafeCell<ManuallyDrop<T>>,
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        Self {
            // Allocate with Box::new, then Box::leak to give up exclusive ownership
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                data_ref_count: AtomicUsize::new(1),
                alloc_ref_count: AtomicUsize::new(1),
                data: UnsafeCell::new(ManuallyDrop::new(data)),
            }))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        // Safety: We know that the pointer will always point to a valid ArcData<T> as long as the
        // Weak<T> exists, because we only deallocate when the ref_count reaches zero,
        // which is only possible when all Arc<T> instances are dropped.
        unsafe { self.ptr.as_ref() }
    }

    // If the reference counter is 1, allow a &mut Self.
    // To do this safely, we need a happens-before relationship for every single drop that led to
    // the reference counter being set to 1.
    // NOTE: this function does not take self, to require Arc::get_mut() and avoid Deref ambiguity.
    // Optimised impl: Check if both counters are set to 1 to be able to determine whether there's
    // only one Arc and no Weak pointers.
    // To avoid missing an upgrade or downgrade, block downgrade by locking the weak pointer counter,
    // using a special value to represent this locked state.
    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        // Acquire matches Weak::drop's Release decrement, to make sure any
        // upgraded pointers are visible in the next data_ref_count.load.
        if arc.data().alloc_ref_count.compare_exchange(
            1, usize::MAX, Acquire, Relaxed).is_err() {
            return None;
        }

        let is_unique = arc.data().data_ref_count.load(Relaxed) == 1;
        // Release matches Acquire increment in `downgrade`, to make sure any
        // changes to the data_ref_count that come after `downgrade` don't
        // change the is_unique result above
        arc.data().alloc_ref_count.store(1, Release);
        if !is_unique {
            return None;
        }

        // Acquire to match Arc::drop's Release decrement, to make sure nothing
        // else is accessing the data.
        fence(Acquire);
        unsafe { Some(&mut *arc.data().data.get()) }
    }

    // Downgrade an Arc to a Weak
    // Checks for the special "lock" value, spins if set
    pub fn downgrade(arc: &Self) -> Weak<T> {
        let mut n = arc.data().alloc_ref_count.load(Relaxed);
        loop {
            if n == usize::MAX {
                // Spin until the lock is released
                std::hint::spin_loop();
                n = arc.data().alloc_ref_count.load(Relaxed);
                continue;
            }
            assert!(n <= usize::MAX / 2);
            // Acquire synchronises with get_mut's release-store.
            if let Err(e) =
                arc.data()
                    .alloc_ref_count
                    .compare_exchange_weak(n, n + 1, Acquire, Relaxed)
            {
                n = e;
                continue;
            }
            return Weak { ptr: arc.ptr };
        }
    }
}

impl<T> Weak<T> {
    #[allow(unused)]
    fn data(&self) -> &ArcData<T> {
        // Safety: We know that the pointer will always point to a valid ArcData<T> as long as the
        // Weak<T> exists, because we only deallocate when the ref_count reaches zero,
        // which is only possible when all Arc<T> instances are dropped.
        unsafe { self.ptr.as_ref() }
    }

    // Update a Weak to an Arc, if the data still exists (At least one Arc exists).
    #[allow(unused)]
    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut n = self.data().data_ref_count.load(Relaxed);
        loop {
            if n == 0 {
                return None;
            }
            assert!(n <= usize::MAX / 2);
            if let Err(e) =
                self.data()
                    .data_ref_count
                    .compare_exchange_weak(n, n + 1, Relaxed, Relaxed)
            {
                n = e;
                continue;
            }
            return Some(Arc { ptr: self.ptr });
        }
    }
}

// Implement Deref, but not DerefMut because we can't unconditionally provide a &mut T.
impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: Since there's an Arc to the data, the data exists and may be shared
        unsafe { &*self.data().data.get() }
    }
}

// Implement Clone, using the same pointer after incrementing the reference counter
// Optimisation: Only needs to touch one counter!
impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().data_ref_count.fetch_add(1, Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { ptr: self.ptr }
    }
}

// Implement Clone, using the same pointer after incrementing the reference counter
impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        // Nothing needs to happen strictly before or after the increment, so Relaxed is sufficient.
        if self.data().alloc_ref_count.fetch_add(1, Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Self { ptr: self.ptr }
    }
}

// Implement Drop, decrementing the data reference counter and dropping the weak if it reaches zero
// Optimisation: Only needs to touch one counter (except for last drop)!
impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.data().data_ref_count.fetch_sub(1, Release) == 1 {
            fence(Acquire);
            // Safety: The data reference counter is zero,
            // so nothing will access the data anymore.
            unsafe {
                ManuallyDrop::drop(&mut *self.data().data.get());
            }
            // Now that there are no more Arcs,
            // drop the implicit weak pointer that represented all Arcs
            drop(Weak { ptr: self.ptr });
        }
    }
}

// Implement Drop, decrementing the alloc reference counter and deallocating if it reaches zero
impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.data().alloc_ref_count.fetch_sub(1, Release) == 1 {
            fence(Acquire);
            // Safety: The data reference counter is zero, so nothing will access it
            unsafe { drop(Box::from_raw(self.ptr.as_ptr())) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);

        struct DetectDrop;

        impl Drop for DetectDrop {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Relaxed);
            }
        }

        // Create an Arc with two weak pointers.
        let x = Arc::new(("hello", DetectDrop));
        let y = Arc::downgrade(&x);
        let z = Arc::downgrade(&x);

        let t = std::thread::spawn(move || {
            // Weak pointer should be upgradeable at this point
            let y = y.upgrade().unwrap();
            assert_eq!(y.0, "hello");
        });

        // In parallel, x should still be available here
        assert_eq!(x.0, "hello");

        // Drop before join for Miri error without fence(Acquire)
        //drop(y);

        t.join().unwrap();

        // The data shouldn't be dropped yet, and the weak pointer should be upgradeable
        assert_eq!(NUM_DROPS.load(Relaxed), 0);
        assert!(z.upgrade().is_some());

        // Drop the Arc
        drop(x);

        // Now the data should be dropped, and the weak pointer should not be upgradeable
        assert_eq!(NUM_DROPS.load(Relaxed), 1);
        assert!(z.upgrade().is_none());
    }

    #[test]
    fn test_get_mut() {
        let mut x = Arc::new(42);
        assert_eq!(Arc::get_mut(&mut x), Some(&mut 42));
        let y = x.clone();
        assert_eq!(Arc::get_mut(&mut x), None);
        drop(y);
        assert_eq!(Arc::get_mut(&mut x), Some(&mut 42));
    }
}

// TODO: long stress-test
