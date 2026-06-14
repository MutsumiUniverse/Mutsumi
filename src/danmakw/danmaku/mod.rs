mod queue;
mod sort;

pub use queue::DanmakuQueue;

#[derive(Debug, Clone, PartialEq)]
pub struct Danmaku {
    pub content: String,
    // milliseconds
    pub start: f64,
    pub color: Color,
    pub mode: DanmakuMode,
}

pub struct ScrollingDanmaku {
    pub danmaku: Danmaku,
    pub texture: gtk::gdk::MemoryTexture,
    /// Distance (px) from the texture edge to the text origin, same on all four sides.
    pub origin_offset: f32,
    pub x: f32,
    pub row: usize,
    pub velocity_x: f32,
    pub width: f32,
}

pub struct CenterDanmaku {
    pub danmaku: Danmaku,
    pub texture: gtk::gdk::MemoryTexture,
    pub origin_offset: f32,
    pub width: f32,
    pub row: usize,
    pub remaining_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum DanmakuMode {
    #[default]
    Scroll,
    TopCenter,
    BottomCenter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

