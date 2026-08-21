mod progress_reporting;
mod lazy_init;

pub fn run_all() {
    //progress_reporting::single_thread();
    //progress_reporting::multiple_threads();
    lazy_init::lazy_init();
}
