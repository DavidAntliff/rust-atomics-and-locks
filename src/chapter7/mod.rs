use std::hint::black_box;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, compiler_fence};
use std::thread;
use std::time::Instant;

static A: AtomicU64 = AtomicU64::new(0);

fn baseline_measurement() {
    black_box(&A);
    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A.load(Relaxed));
    }
    println!("baseline_measurement: {:?}", start.elapsed());
}

fn cache_measurement_read_only() {
    black_box(&A);

    thread::spawn(|| {
        loop {
            black_box(A.load(Relaxed));
        }
    });

    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A.load(Relaxed));
    }
    println!("cache_measurement_read_only: {:?}", start.elapsed());
}

fn cache_measurement_with_background_store() {
    black_box(&A);

    thread::spawn(|| {
        loop {
            A.store(0, Relaxed);
        }
    });

    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A.load(Relaxed));
    }
    println!("cache_measurement_with_background_store: {:?}", start.elapsed());
}

fn cache_measurement_with_compare_and_exchange() {
    black_box(&A);

    // Instead of a store, use a compare_exchange (that always fails)
    thread::spawn(|| {
        loop {
            // Never succeeds because A is never 10.
            // This doesn't store anything, but it will still claim exclusive access of the
            // cache line, affecting the main thread's load.
            // Swap does a similar thing.
            // Therefore, it can be more efficient in a spin loop to use a load first, to check
            // if the value is what is expected, and then only do the more expensive
            // compare_exchange if it is.
            black_box(A.compare_exchange(10, 20, Relaxed, Relaxed).is_ok());
        }
    });

    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A.load(Relaxed));
    }
    println!("cache_measurement_with_background_store: {:?}", start.elapsed());
}

static B: [AtomicU64; 3] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

fn cache_measurement_with_false_sharing() {
    black_box(&B);
    thread::spawn(|| {
        loop {
            B[0].store(0, Relaxed);
            B[2].store(0, Relaxed);
        }
    });
    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(B[1].load(Relaxed));
    }
    println!("cache_measurement_with_false_sharing: {:?}", start.elapsed());
}

#[repr(align(64))] // This struct must be 64-byte aligned.
struct Aligned64(AtomicU64);

static C: [Aligned64; 3] = [
    Aligned64(AtomicU64::new(0)),
    Aligned64(AtomicU64::new(0)),
    Aligned64(AtomicU64::new(0)),
];

fn cache_measurement_with_64_byte_cache_alignment() {
    black_box(&C);
    thread::spawn(|| {
        loop {
            C[0].0.store(1, Relaxed);
            C[2].0.store(1, Relaxed);
        }
    });
    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(C[1].0.load(Relaxed));
    }
    println!("cache_measurement_with_64_byte_cache_alignment: {:?}", start.elapsed());
}

#[repr(align(128))] // This struct must be 128-byte aligned.
struct Aligned128(AtomicU64);

static D: [Aligned128; 3] = [
    Aligned128(AtomicU64::new(0)),
    Aligned128(AtomicU64::new(0)),
    Aligned128(AtomicU64::new(0)),
];

fn cache_measurement_with_128_byte_cache_alignment() {
    black_box(&D);
    thread::spawn(|| {
        loop {
            D[0].0.store(1, Relaxed);
            D[2].0.store(1, Relaxed);
        }
    });
    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(D[1].0.load(Relaxed));
    }
    println!("cache_measurement_with_128_byte_cache_alignment: {:?}", start.elapsed());
}

// Deliberately broken lock implementation, to demonstrate how memory ordering can break things.
// Should work fine on x86_64, but may fail on aarch64 due to weak memory ordering.
fn deliberately_broken_lock() {
    let locked = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);

    thread::scope(|s| {
        // Spawn four threads, that each iterate a million times.
        for _ in 0..4 {
            s.spawn(|| for _ in 0..1_000_000 {
                // Acquire the lock, using the wrong memory ordering.
                while locked.swap(true, Relaxed) {}
                compiler_fence(Acquire);

                // Non-atomically increment the counter, while holding the lock.
                let old = counter.load(Relaxed);
                let new = old + 1;
                counter.store(new, Relaxed);

                // Release the lock, using the wrong memory ordering.
                compiler_fence(Release);
                locked.store(false, Relaxed);
            });
        }
    });

    println!("deliberately_broken_lock: {}", counter.into_inner());
}

// Deliberately broken lock implementation, to demonstrate how memory ordering can break things.
// Should work fine on x86_64, but may fail on aarch64 due to weak memory ordering.
fn correct_lock() {
    let locked = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);

    thread::scope(|s| {
        // Spawn four threads, that each iterate a million times.
        for _ in 0..4 {
            s.spawn(|| for _ in 0..1_000_000 {
                // Acquire the lock, using the correct memory ordering.
                while locked.swap(true, Acquire) {}

                // Non-atomically increment the counter, while holding the lock.
                let old = counter.load(Relaxed);
                let new = old + 1;
                counter.store(new, Relaxed);

                // Release the lock, using the correct memory ordering.
                locked.store(false, Release);
            });
        }
    });

    println!("correct_lock: {}", counter.into_inner());
}

pub fn run_all() {
    baseline_measurement();
    cache_measurement_read_only();
    cache_measurement_with_background_store();
    cache_measurement_with_compare_and_exchange();
    cache_measurement_with_false_sharing();
    cache_measurement_with_64_byte_cache_alignment();
    cache_measurement_with_128_byte_cache_alignment();
    deliberately_broken_lock();
    correct_lock();
}
