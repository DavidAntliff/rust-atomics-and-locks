use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::thread;
use std::time::{Duration, Instant};

pub fn single_thread() {
    let num_done = AtomicUsize::new(0);

    let main_thread = thread::current();

    thread::scope(|s| {
        // A background thread to process all 100 items.
        s.spawn(|| {
            for i in 0..100 {
                process_item(i); // Assuming this takes some time.
                num_done.store(i + 1, Relaxed);
                main_thread.unpark(); // Wake up the main thread.
            }
        });

        // The main thread shows status updates.
        loop {
            let n = num_done.load(Relaxed);
            if n == 100 { break; }
            println!("Working.. {n}/100 done");
            thread::park_timeout(Duration::from_secs(1));
        }
    });

    println!("Done!");
}

pub fn multiple_threads() {
    // Needs to be shared (copied) between threads, so we use a reference to an AtomicUsize.
    let num_done = &AtomicUsize::new(0);
    let total_time = &AtomicU64::new(0);
    let max_time = &AtomicU64::new(0);

    let main_thread = &thread::current();

    thread::scope(|s| {
        // Four background threads to process 100 items.
        for t in 0..4 {
            // Must move as t is temporary
            s.spawn(move || {
                for i in 0..25 {
                    let start = Instant::now();
                    process_item(t * 25 + i); // takes time
                    let time_taken = start.elapsed().as_micros() as u64;
                    num_done.fetch_add(1, Relaxed);
                    total_time.fetch_add(time_taken, Relaxed);
                    max_time.fetch_max(time_taken, Relaxed);
                    main_thread.unpark(); // Wake up the main thread.
                }
            });
        }

        // The main thread shows status updates.
        loop {
            let total_time = Duration::from_micros(total_time.load(Relaxed));
            let max_time = Duration::from_micros(max_time.load(Relaxed));
            let n = num_done.load(Relaxed);
            if n == 100 { break; }
            if n == 0 {
                println!("Nothing done yet...");
            } else {
                println!("Working.. {n}/100 done, {:?} average, {:?} peak",
                         total_time / n as u32,
                         max_time);
            }
            thread::park_timeout(Duration::from_secs(1));
        }
    });

    println!("Done!");
}

fn process_item(x: usize) {
    // hash x into a value between 20 and 70 to simulate variable work time
    let h = x.wrapping_mul(0x9E3779B97F4A7C15).rotate_left(32);
    let d = (h % 50) + 20;
    thread::sleep(Duration::from_millis(d as u64)); // Simulate work
}
