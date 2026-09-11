//! Arc implementation with Weak pointers

use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicUsize, fence};

// Use a pointer and handle allocation and ownership manually
pub struct Arc<T> {
    weak: Weak<T>,
}

// An object responsible for keeping an ArcData<T> alive
pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}

// Internal details, which will be shared
struct ArcData<T> {
    /// Number of Arcs
    data_ref_count: AtomicUsize,
    /// Number of Arcs and Weaks combined
    alloc_ref_count: AtomicUsize,
    /// The data. None if there are only weak pointers left.
    data: UnsafeCell<Option<T>>,
}

// Arc<T> should be Send iff T is both Send & Sync (sending to another thread might result in drop).
// Arc<T> should be Sync iff T is both Send & Sync (multiple threads can access the same Arc<T>).
unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        Self {
            weak: Weak {
                // Allocate with Box::new, then Box::leak to give up exclusive ownership
                ptr: NonNull::from(Box::leak(Box::new(ArcData {
                    data_ref_count: AtomicUsize::new(1),
                    alloc_ref_count: AtomicUsize::new(1),
                    data: UnsafeCell::new(Some(data)),
                }))),
            },
        }
    }

    fn data(&self) -> &ArcData<T> {
        self.weak.data()
    }

    // If the reference counter is 1, allow a &mut Self.
    // To do this safely, we need a happens-before relationship for every single drop that led to
    // the reference counter being set to 1.
    // NOTE: this function does not take self, to require Arc::get_mut() and avoid Deref ambiguity.
    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.weak.data().alloc_ref_count.load(Relaxed) == 1 {
            fence(Acquire);
            // Safety: Nothing else can access data, since there is only one Arc,
            // to which we have exclusive access, and no Weak pointers.
            let arcdata = unsafe { arc.weak.ptr.as_mut() };
            let option = arcdata.data.get_mut();
            // We know the data is still available since we have an Arc to it, so this won't panic
            let data = option.as_mut().unwrap();
            Some(data)
        } else {
            None
        }
    }

    // Downgrade an Arc to a Weak
    pub fn downgrade(arc: &Self) -> Weak<T> {
        arc.weak.clone()
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        // Safety: We know that the pointer will always point to a valid ArcData<T> as long as the
        // Weak<T> exists, because we only deallocate when the ref_count reaches zero,
        // which is only possible when all Arc<T> instances are dropped.
        unsafe { self.ptr.as_ref() }
    }

    // Update a Weak to an Arc, if the data still exists (At least one Arc exists).
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
            return Some(Arc { weak: self.clone() });
        }
    }
}

// Implement Deref, but not DerefMut because we can't unconditionally provide a &mut T.
impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let ptr = self.weak.data().data.get();
        // Safety: Since there's an Arc to the data, the data exists and may be shared
        unsafe { (*ptr).as_ref().unwrap() }
    }
}

// Implement Clone, using the same pointer after incrementing the reference counter
impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        // TODO: Handle overflows better: https://mara.nl/atomics/atomics.html#example-handle-overflow
        let weak = self.weak.clone();
        if weak.data().data_ref_count.fetch_add(1, Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { weak }
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
impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        // The final fetch_sub must establish a happens-before relationship with every previous fetch_sub.
        // Decrementing to a non-zero value is "release", but decrementing to zero is "acquire".
        // So we could use AcqRel ordering for all decrements, but this is inefficient.
        if self.weak.data().data_ref_count.fetch_sub(1, Release) == 1 {
            // Instead, use Release for all decrements for efficiency,
            // and then explicitly use an Acquire fence if decrementing to zero:
            fence(Acquire);
            let ptr = self.weak.data().data.get();
            // Safety: The data reference counter is zero,
            // so nothing will access it.
            unsafe {
                (*ptr) = None;
            }
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
