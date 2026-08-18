pub mod cell;
pub mod thread;

pub fn run_all() {
    cell::cell_to_thread();
    cell::atomic_to_thread();

    thread::parking();
    thread::condition_var();
    thread::poisoning();
    thread::arc_sharing();
}
