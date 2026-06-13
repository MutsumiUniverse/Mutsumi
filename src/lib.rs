pub mod control;
mod danmakw;
pub mod video;

pub use control::*;
pub use video::*;
pub use danmakw::*;

pub fn init() {
    control::init();
    danmakw::init();
}
