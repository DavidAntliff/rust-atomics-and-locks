mod cell;
mod thread;

fn main() {
    cell::cell_to_thread();
    cell::atomic_to_thread();

    thread::parking();
    thread::condition_var();
    thread::poisoning();
    thread::arc_sharing();
}
