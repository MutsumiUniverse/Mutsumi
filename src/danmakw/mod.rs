mod pool;
mod timer;
mod layout;
mod danmaku;
mod alloc;
mod render;

pub use pool::*;
pub use layout::*;
pub use timer::*;
pub use alloc::*;
pub use danmaku::*;
pub use render::*;

use gtk::prelude::*;

pub fn init() {
    Danmakw::ensure_type();
}
