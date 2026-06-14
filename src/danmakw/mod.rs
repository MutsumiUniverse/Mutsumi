mod pool;
mod timer;
mod layout;
mod danmaku;
mod alloc;
mod render;
mod parse;

pub use pool::*;
pub use layout::*;
pub use timer::*;
pub use alloc::*;
pub use danmaku::*;
pub use render::*;
pub use parse::*;

use gtk::prelude::*;

pub fn init() {
    Danmakw::ensure_type();
}
