use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicUsize, fence};

// Use a pointer and handle allocation and ownership manually
pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

// Internal details, which will be shared
struct ArcData<T> {
    ref_count: AtomicUsize,
    data: T,
}

// Arc<T> should be Send iff T is both Send & Sync (sending to another thread might result in drop).
// Arc<T> should be Sync iff T is both Send & Sync (multiple threads can access the same Arc<T>).
unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        Self {
            // Allocate with Box::new, then Box::leak to give up exclusive ownership
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                ref_count: AtomicUsize::new(1),
                data,
            }))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        // Safety: We know that the pointer will always point to a valid ArcData<T> as long as the
        // Arc<T> exists, because we only deallocate when the ref_count reaches zero,
        // which is only possible when all Arc<T> instances are dropped.
        unsafe { self.ptr.as_ref() }
    }

    // If the reference counter is 1, allow a &mut Self.
    // To do this safely, we need a happens-before relationship for every single drop that led to
    // the reference counter being set to 1.
    // NOTE: this function does not take self, to require Arc::get_mut() and avoid Deref ambiguity.
    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.data().ref_count.load(Relaxed) == 1 {
            fence(Acquire);
            // Safety: Nothing else can access data, since there is only one Arc,
            // to which we have exclusive access.
            unsafe { Some(&mut arc.ptr.as_mut().data) }
        } else {
            None
        }
    }
}

// Implement Deref, but not DerefMut because we can't unconditionally provide a &mut T.
impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data().data
    }
}

// Implement Clone, using the same pointer after incrementing the reference counter
impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        // TODO: Handle overflows better: https://mara.nl/atomics/atomics.html#example-handle-overflow
        // Nothing needs to happen strictly before or after the increment, so Relaxed is sufficient.
        if self.data().ref_count.fetch_add(1, Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Self { ptr: self.ptr }
    }
}

// Implement Drop, decrementing the reference counter and deallocating if it reaches zero
impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        // The final fetch_sub must establish a happens-before relationship with every previous fetch_sub.
        // Decrementing to a non-zero value is "release", but decrementing to zero is "acquire".
        // So we could use AcqRel ordering for all decrements, but this is inefficient.
        if self.data().ref_count.fetch_sub(1, Release) == 1 {
            // Instead, use Release for all decrements for efficiency,
            // and then explicitly use an Acquire fence if decrementing to zero:
            fence(Acquire);

            // Safety: We know that the pointer will always point to a valid ArcData<T> as long as the
            // Arc<T> exists, because we only deallocate when the ref_count reaches zero,
            // which is only possible when all Arc<T> instances are dropped.
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
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

        // Create two Arcs sharing an object containing a string
        // and a DetectDrop, to detect when it is dropped.
        let x = Arc::new(("hello", DetectDrop));
        let y = x.clone();

        // Send x to another thread and use it there
        let t = std::thread::spawn(move || {
            assert_eq!(x.0, "hello");
        });

        // In parallel, y should still be available here
        assert_eq!(y.0, "hello");

        // Drop before join for Miri error without fence(Acquire)
        //drop(y);

        t.join().unwrap();

        // One Arc, x, should have dropped by now
        // but the other, y, should still be alive
        assert_eq!(NUM_DROPS.load(Relaxed), 0);

        // Drop the remaining Arc
        drop(y);

        // Now the DetectDrop should have been dropped
        assert_eq!(NUM_DROPS.load(Relaxed), 1);
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
