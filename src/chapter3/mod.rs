mod relaxed;
mod acquire_release;
mod lazy_init;

pub fn run_all() {
    relaxed::happens_before();
    acquire_release::acquire_release();
    lazy_init::lazy_init_with_indirection();
}
