pub mod control;
mod danmakw;
pub mod video;

pub use control::*;
pub use danmakw::*;
pub use video::*;

pub fn init() {
    unsafe { std::env::set_var("GSK_RENDERER", "gl"); }

    control::init();
    danmakw::init();
}
