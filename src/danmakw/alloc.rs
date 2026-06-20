use super::*;
use gtk::gdk;
use pango::{Context, Layout};

const SCROLL_DURATION_MS: f32 = 10000.0;
const CENTER_DURATION_MS: f32 = 5000.0;
const RESET_DELTA_MS: f32 = 1000.0;
const SEEK_PREROLL_STEP_MS: f64 = 50.0;

fn make_texture(
    layout: &Layout,
    color: &Color,
    outline_px: f64,
    shadow_offset: f64,
) -> (gdk::MemoryTexture, f32) {
    let (pw, ph) = layout.pixel_size();
    let pad = (outline_px + shadow_offset).ceil() as i32;
    let w = pw + 2 * pad;
    let h = ph + 2 * pad;

    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
        .expect("Failed to create cairo surface???");
    {
        let cr = cairo::Context::new(&surface).expect("Failed to create cairo context???");
        let ox = outline_px;
        let oy = outline_px;

        cr.move_to(ox + shadow_offset, oy + shadow_offset);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.65);
        pangocairo::functions::show_layout(&cr, layout);

        cr.move_to(ox, oy);
        pangocairo::functions::layout_path(&cr, layout);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.set_line_width(outline_px * 2.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.95);
        cr.stroke().unwrap();

        cr.move_to(ox, oy);
        cr.set_source_rgba(
            color.r as f64 / 255.0,
            color.g as f64 / 255.0,
            color.b as f64 / 255.0,
            color.a as f64 / 255.0,
        );
        pangocairo::functions::show_layout(&cr, layout);
    }
    surface.flush();

    let stride = surface.stride() as usize;
    let tex = {
        let data = surface.data().expect("Failed to get cairo surface data???");
        let bytes = glib::Bytes::from(&*data);
        gdk::MemoryTexture::new(
            w,
            h,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        )
    };
    (tex, outline_px as f32)
}

pub struct DanmakwRenderer {
    pub danmaku_queue: DanmakuQueue,
    pub last_time: f64,

    pub paused: bool,

    pub scroll_danmaku: Vec<ScrollingDanmaku>,
    pub scroll_max_rows: usize,

    pub top_center_danmaku: Vec<CenterDanmaku>,
    pub top_center_max_rows: usize,
    pub top_center_row_occupied: Vec<bool>,

    pub bottom_center_danmaku: Vec<CenterDanmaku>,
    pub bottom_center_max_rows: usize,
    pub bottom_center_row_occupied: Vec<bool>,

    pub line_height: f32,
    pub top_padding: f32,
    /// Font size in logical pt (from UI).
    pub font_size: f64,
    pub font_name: String,
    pub font_weight: pango::Weight,
    /// Spacing between danmaku rows as a multiple of font height (logical px).
    pub spacing_factor: f32,
    /// Cached spacing in logical px, recomputed in add_danmaku.
    spacing: f32,
    pub outline_px: f64,
    pub shadow_offset: f64,
    pub scale_factor: f64,
    pub speed_factor: f64,
}

impl Default for DanmakwRenderer {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl DanmakwRenderer {
    pub fn new(scale_factor: f64) -> Self {
        let scroll_max_rows = 25;
        let top_center_max_rows = 5;
        let bottom_center_max_rows = 5;
        let font_size = 24.0_f64;
        let font_px_logical = font_size * (96.0 / 72.0);
        let spacing_factor = 1.5_f32;
        let line_height = font_px_logical as f32 * spacing_factor;
        let spacing = font_px_logical as f32;
        let top_padding = 10.0;

        let top_center_row_occupied = vec![false; top_center_max_rows];
        let bottom_center_row_occupied = vec![false; bottom_center_max_rows];

        Self {
            font_name: String::new(),
            font_size,
            font_weight: pango::Weight::Normal,
            spacing_factor,
            spacing,
            outline_px: 1.0,
            shadow_offset: 1.0,
            danmaku_queue: DanmakuQueue::new(),
            scroll_danmaku: Vec::new(),
            top_center_danmaku: Vec::new(),
            bottom_center_danmaku: Vec::new(),
            scroll_max_rows,
            top_center_max_rows,
            bottom_center_max_rows,
            line_height,
            top_padding,
            scale_factor,
            speed_factor: 1.0,
            top_center_row_occupied,
            bottom_center_row_occupied,
            paused: false,
            last_time: 0.0,
        }
    }

    pub fn add_scroll_danmaku(
        &mut self,
        layout: Layout,
        text_width: f32,
        width: f32,
        danmaku: Danmaku,
    ) {
        let velocity_x = -(width + text_width) / SCROLL_DURATION_MS * self.speed_factor as f32;

        let v = velocity_x.abs();

        let mut found_row: Option<usize> = None;

        let reach_edge_time = width / v;

        for target_row in 0..self.scroll_max_rows {
            let last_in_row = self
                .scroll_danmaku
                .iter()
                .filter(|d| d.row == target_row)
                .max_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

            let Some(last_in_row) = last_in_row else {
                found_row = Some(target_row);
                break;
            };

            let leave_time =
                (last_in_row.x + last_in_row.width + self.spacing) / last_in_row.velocity_x.abs();

            if leave_time < reach_edge_time
                && width > last_in_row.width + self.spacing + last_in_row.x
            {
                found_row = Some(target_row);
                break;
            }
        }

        let Some(target_row) = found_row else {
            return;
        };

        let (texture, origin_offset_dev) =
            make_texture(&layout, &danmaku.color, self.outline_px, self.shadow_offset);
        let origin_offset = origin_offset_dev / self.scale_factor as f32;
        self.scroll_danmaku.push(ScrollingDanmaku {
            danmaku,
            texture,
            origin_offset,
            x: width,
            row: target_row,
            velocity_x,
            width: text_width,
        });
    }

    pub fn add_topcenter_danmaku(&mut self, layout: Layout, text_width: f32, danmaku: Danmaku) {
        let Some(target_row) = self
            .top_center_row_occupied
            .iter()
            .position(|&occupied| !occupied)
        else {
            return;
        };

        self.top_center_row_occupied[target_row] = true;

        let (texture, origin_offset_dev) =
            make_texture(&layout, &danmaku.color, self.outline_px, self.shadow_offset);
        let origin_offset = origin_offset_dev / self.scale_factor as f32;
        self.top_center_danmaku.push(CenterDanmaku {
            danmaku,
            texture,
            origin_offset,
            width: text_width,
            row: target_row,
            remaining_time: CENTER_DURATION_MS,
        });
    }

    pub fn add_bottomcenter_danmaku(&mut self, layout: Layout, text_width: f32, danmaku: Danmaku) {
        let Some(target_row) = self
            .bottom_center_row_occupied
            .iter()
            .position(|&occupied| !occupied)
        else {
            return;
        };

        self.bottom_center_row_occupied[target_row] = true;

        let (texture, origin_offset_dev) =
            make_texture(&layout, &danmaku.color, self.outline_px, self.shadow_offset);
        let origin_offset = origin_offset_dev / self.scale_factor as f32;
        self.bottom_center_danmaku.push(CenterDanmaku {
            danmaku,
            texture,
            origin_offset,
            width: text_width,
            row: target_row,
            remaining_time: CENTER_DURATION_MS,
        });
    }

    pub fn rebuild_visible_state_at(
        &mut self,
        context: &Context,
        screen_width: f32,
        time_milis: f64,
    ) {
        let preroll_ms = SCROLL_DURATION_MS.max(CENTER_DURATION_MS) as f64;
        let start_time = (time_milis - preroll_ms).max(0.0);

        self.scroll_danmaku.clear();
        self.top_center_danmaku.clear();
        self.bottom_center_danmaku.clear();
        self.top_center_row_occupied.fill(false);
        self.bottom_center_row_occupied.fill(false);

        self.danmaku_queue.reset_time(start_time);
        self.last_time = start_time;

        let mut simulated_time = start_time;
        while simulated_time + SEEK_PREROLL_STEP_MS < time_milis {
            simulated_time += SEEK_PREROLL_STEP_MS;
            self.update(context, screen_width, simulated_time);
        }

        self.update(context, screen_width, time_milis);
    }

    pub fn update(&mut self, context: &Context, screen_width: f32, time_milis: f64) {
        let delta_time = (time_milis - self.last_time) as f32;
        self.last_time = time_milis;

        if delta_time.abs() > RESET_DELTA_MS {
            self.danmaku_queue.reset_time(time_milis);
            return;
        }

        let mut danmaku_queue = std::mem::take(&mut self.danmaku_queue);
        for next_danmaku in danmaku_queue.pop_to_time_iter(time_milis).take(8) {
            self.add_danmaku(context, screen_width, next_danmaku.clone());
        }
        self.danmaku_queue = danmaku_queue;

        for text in self.scroll_danmaku.iter_mut() {
            text.x += text.velocity_x * delta_time * self.speed_factor as f32;
        }

        self.scroll_danmaku.retain(|text| text.x + text.width > 0.0);

        for text in self.top_center_danmaku.iter_mut() {
            text.remaining_time -= delta_time;
        }

        self.top_center_danmaku.retain(|text| {
            if text.remaining_time <= 0.0 {
                if let Some(occupied) = self.top_center_row_occupied.get_mut(text.row) {
                    *occupied = false;
                }
                false
            } else {
                true
            }
        });

        for text in self.bottom_center_danmaku.iter_mut() {
            text.remaining_time -= delta_time;
        }

        self.bottom_center_danmaku.retain(|text| {
            if text.remaining_time <= 0.0 {
                if let Some(occupied) = self.bottom_center_row_occupied.get_mut(text.row) {
                    *occupied = false;
                }
                false
            } else {
                true
            }
        });
    }

    pub fn add_danmaku(&mut self, context: &Context, screen_width: f32, danmaku: Danmaku) {
        let raw_dpi = pangocairo::functions::context_get_resolution(context);
        let dpi = if raw_dpi > 0.0 { raw_dpi } else { 96.0 };
        // font_size is in logical pt; convert to device px for absolute rendering.
        let font_px_device = self.font_size * (dpi / 72.0) * self.scale_factor;
        // Logical px (used for layout positions and spacing).
        let font_px_logical = self.font_size * (dpi / 72.0);
        // spacing_factor controls vertical row height (上下间距 = N × font size).
        self.line_height = font_px_logical as f32 * self.spacing_factor;
        // Fixed horizontal gap between consecutive scrolling danmaku (1× font size).
        self.spacing = font_px_logical as f32;

        let layout = Layout::new(context);
        let mut font_desc = pango::FontDescription::default();
        font_desc.set_absolute_size(font_px_device * pango::SCALE as f64);
        font_desc.set_family(&self.font_name);
        font_desc.set_weight(self.font_weight);
        layout.set_font_description(Some(&font_desc));
        layout.set_text(&danmaku.content);

        // layout.pixel_size() is in device px; convert to logical for positions.
        let text_width = layout.pixel_size().0 as f32 / self.scale_factor as f32;

        match danmaku.mode {
            DanmakuMode::Scroll => {
                self.add_scroll_danmaku(layout, text_width, screen_width, danmaku);
            }
            DanmakuMode::TopCenter => {
                self.add_topcenter_danmaku(layout, text_width, danmaku);
            }
            DanmakuMode::BottomCenter => {
                self.add_bottomcenter_danmaku(layout, text_width, danmaku);
            }
        }
    }

    pub fn clear_danmaku(&mut self) {
        self.scroll_danmaku.clear();
        self.top_center_danmaku.clear();
        self.bottom_center_danmaku.clear();
        self.top_center_row_occupied.fill(false);
        self.bottom_center_row_occupied.fill(false);
    }

    pub fn scrolled_top_y(&self, row: usize) -> f32 {
        self.top_padding + row as f32 * self.line_height
    }

    pub fn top_center_y(&self, row: usize) -> f32 {
        self.top_padding + row as f32 * self.line_height
    }

    pub fn bottom_center_y(&self, row: usize, screen_height: f32) -> f32 {
        screen_height - self.top_padding - (row + 1) as f32 * self.line_height
    }

    pub fn set_font_weight_index(&mut self, index: u32) {
        self.font_weight = Self::pango_weight_from_index(index);
    }

    fn pango_weight_from_index(index: u32) -> pango::Weight {
        match index {
            0 => pango::Weight::Thin,
            1 => pango::Weight::Ultralight,
            2 => pango::Weight::Light,
            3 => pango::Weight::Semilight,
            4 => pango::Weight::Book,
            5 => pango::Weight::Normal,
            6 => pango::Weight::Medium,
            7 => pango::Weight::Semibold,
            8 => pango::Weight::Bold,
            9 => pango::Weight::Ultrabold,
            10 => pango::Weight::Heavy,
            11 => pango::Weight::Ultraheavy,
            _ => pango::Weight::Normal,
        }
    }

    pub fn set_intensity(&mut self, index: u32) {
        let (scroll, center) = Self::max_rows_from_intensity(index);
        self.scroll_max_rows = scroll;
        self.top_center_max_rows = center;
        self.bottom_center_max_rows = center;
        self.top_center_row_occupied.resize(center, false);
        self.bottom_center_row_occupied.resize(center, false);
    }

    fn max_rows_from_intensity(index: u32) -> (usize, usize) {
        match index {
            0 => (12, 2),     // Quarter
            1 => (25, 5),     // Half
            2 => (50, 10),    // Full
            _ => (9999, 999), // Allow Overlay
        }
    }
}
