pub mod control;
mod danmakw;
pub mod video;

pub use control::*;
pub use danmakw::*;
pub use video::*;

pub fn init() {
    control::init();
    danmakw::init();
}
