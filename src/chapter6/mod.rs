// To test with Miri:
//
//   MIRIFLAGS="-Zmiri-many-seeds -Zmiri-preemption-rate=0.9" cargo miri test chapter6::arc_weak

pub mod arc;
pub mod arc_weak;
pub mod arc_weak_opt;

pub fn run_all() {}
