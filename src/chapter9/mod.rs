use std::fmt::Debug;
use std::ops::DerefMut;
use std::thread;
use std::time::Instant;

pub mod mutex_simple;
pub mod mutex_optimised;
pub mod mutex_optimised_more;

trait Mutex {
    type Value;
    type Guard<'a>: Debug + Send + Sync + DerefMut<Target=Self::Value>
    where
        Self: 'a;

    fn lock(&self) -> Self::Guard<'_>;
}

impl<T: Debug + Send + Sync> Mutex for mutex_simple::Mutex<T> {
    type Value = T;
    type Guard<'a> = mutex_simple::MutexGuard<'a, T>
    where
        T: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        mutex_simple::Mutex::lock(self)
        //<mutex_simple::Mutex<Self::Value>>::lock(self)
    }
}

impl<T: Debug + Send + Sync> Mutex for mutex_optimised::Mutex<T> {
    type Value = T;
    type Guard<'a> = mutex_optimised::MutexGuard<'a, T>
    where
        T: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        mutex_optimised::Mutex::lock(self)
    }
}

impl<T: Debug + Send + Sync> Mutex for mutex_optimised_more::Mutex<T> {
    type Value = T;
    type Guard<'a> = mutex_optimised_more::MutexGuard<'a, T>
    where
        T: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        mutex_optimised_more::Mutex::lock(self)
    }
}

const ITERATIONS: usize = 1_000_000;

fn bench<T>(mutex: &T, threads: usize, iterations: usize) -> std::time::Duration
where
    T: Mutex<Value=i32> + Sync,
{
    std::hint::black_box(&mutex);
    let start = Instant::now();
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for _ in 0..iterations {
                    *mutex.lock() += 1;
                }
            });
        }
    });
    start.elapsed()
}

fn demo_mutex<T>(mutex: T)
where
    T: Mutex<Value=i32> + Sync,
{
    println!("=== {} ===", std::any::type_name::<T>());

    // Warm-up: gets pages faulted in and the CPU out of a low clock state.
    bench(&mutex, 1, 10_000);

    let uncontended = bench(&mutex, 1, ITERATIONS);
    println!(
        "  uncontended: {uncontended:?} total, {:?}/op",
        uncontended / ITERATIONS as u32
    );

    for threads in [2, 4, thread::available_parallelism().map_or(8, |n| n.get())] {
        let per_thread = ITERATIONS / threads;
        let elapsed = bench(&mutex, threads, per_thread);
        let ops = (threads * per_thread) as u32;
        println!(
            "  {threads:>2} threads: {elapsed:?} total, {:?}/op",
            elapsed / ops
        );
    }
}

pub fn run_all() {
    demo_mutex(mutex_simple::Mutex::new(0));
    demo_mutex(mutex_optimised::Mutex::new(0));
    demo_mutex(mutex_optimised_more::Mutex::new(0));
}
