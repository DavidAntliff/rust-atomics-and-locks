use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::thread;

pub fn lazy_init() {
    rand::random::<u64>(); // Just to make sure the random number generator is initialized.

    // Needs to be shared (copied) between threads, so we use a reference to an AtomicUsize.
    let wait = &AtomicBool::new(true);

    thread::scope(|s| {
        // A thousand background threads need to agree on a key...
        for t in 0..100 {
            // Must move as t is temporary
            s.spawn(move || {
                while wait.load(Relaxed) {
                    thread::yield_now();
                }

                let key = get_key();
                println!("Thread {t} got key {key:?}");
            });
        }

        thread::sleep(std::time::Duration::from_secs(1));
        wait.store(false, Relaxed);
    });

    let key = get_key();
    println!("key is {key:?}, Done!");
}

fn get_key() -> u64 {
    //get_key_with_store()
    get_key_with_compare_exchange()
}

/// Race condition: multiple threads may generate a new key and overwrite each other.
/// Callers may end up with the wrong key.
fn get_key_with_store() -> u64 {
    static KEY: AtomicU64 = AtomicU64::new(0);
    let key = KEY.load(Relaxed);
    if key == 0 {
        let new_key = generate_random_key();
        // Storing isn't atomic with respect to the load above, so multiple threads
        // may generate a new key and overwrite each other.
        KEY.store(new_key, Relaxed);
        new_key
    } else {
        key
    }
}

/// Race condition: multiple threads generate a new key, but only one will succeed in storing it.
/// Losers will discard their generated key and use the winner's instead.
fn get_key_with_compare_exchange() -> u64 {
    static KEY: AtomicU64 = AtomicU64::new(0);
    let key = KEY.load(Relaxed);
    if key == 0 {
        let new_key = generate_random_key();
        // Compare-and-swap: if the key is still 0, set it to new_key.
        // Otherwise, return the current value.
        match KEY.compare_exchange(0, new_key, Relaxed, Relaxed) {
            Ok(_) => new_key, // We successfully set the key.
            Err(k) => k, // Another thread set the key first, discard ours.
        }
    } else {
        key
    }
}

fn generate_random_key() -> u64 {
    // A placeholder for a real (but cheap) random key generation.
    rand::random()
}
