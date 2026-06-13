pub mod control;
mod danmaku;
pub mod video;

pub use control::*;
pub use video::*;

pub fn control_init() {
    control::init();
}
