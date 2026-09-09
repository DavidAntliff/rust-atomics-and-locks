pub mod simple_mutex_channel;
pub mod unsafe_one_shot_channel;
pub mod one_shot_channel_checked;
pub mod type_safe_channel;
pub mod no_allocation_channel;
pub mod blocking_channel;

pub fn run_all() {
    simple_mutex_channel::demo();
    one_shot_channel_checked::demo();
    type_safe_channel::demo();
    no_allocation_channel::demo();
    blocking_channel::demo();
}
