mod pool;
mod timer;
mod layout;

pub use pool::*;
pub use layout::*;
pub use timer::*;

use gtk::prelude::*;

pub fn init() {
    Danmakw::ensure_type();
}
