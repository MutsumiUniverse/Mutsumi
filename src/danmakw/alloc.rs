use super::*;
use flume::{Receiver, Sender};
use gtk::gdk;
use mutsumi_utils::spawn_tokio_blocking;
use pango::{Context, Layout};

const SCROLL_DURATION_MS: f32 = 10000.0;
const CENTER_DURATION_MS: f32 = 5000.0;
const RESET_DELTA_MS: f32 = 1000.0;
const SEEK_PREROLL_STEP_MS: f64 = 50.0;

struct TextureRenderParams {
    text: String,
    font_family: String,
    font_weight: pango::Weight,
    font_px_device: f64,
    dpi: f64,
    color: Color,
    outline_px: f64,
    shadow_offset: f64,
}

struct PendingScrollRow {
    id: u64,
    row: usize,
    x: f32,
    velocity_x: f32,
    width: f32,
}

enum TextureReady {
    Scroll {
        id: u64,
        bytes: Vec<u8>,
        w: i32,
        h: i32,
        stride: usize,
        danmaku: Danmaku,
        origin_offset: f32,
        velocity_x: f32,
        width: f32,
        row: usize,
    },
    TopCenter {
        bytes: Vec<u8>,
        w: i32,
        h: i32,
        stride: usize,
        danmaku: Danmaku,
        origin_offset: f32,
        width: f32,
        row: usize,
    },
    BottomCenter {
        bytes: Vec<u8>,
        w: i32,
        h: i32,
        stride: usize,
        danmaku: Danmaku,
        origin_offset: f32,
        width: f32,
        row: usize,
    },
}

fn render_texture_raw(params: TextureRenderParams) -> (Vec<u8>, i32, i32, usize) {
    let dummy = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)
        .expect("Failed to create dummy cairo surface");
    let cr_dummy = cairo::Context::new(&dummy).expect("Failed to create dummy cairo context");
    let context = pangocairo::functions::create_context(&cr_dummy);
    pangocairo::functions::context_set_resolution(&context, params.dpi);

    let layout = pango::Layout::new(&context);
    let mut font_desc = pango::FontDescription::default();
    font_desc.set_absolute_size(params.font_px_device * pango::SCALE as f64);
    font_desc.set_family(&params.font_family);
    font_desc.set_weight(params.font_weight);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(&params.text);

    let (pw, ph) = layout.pixel_size();
    let pad = (params.outline_px + params.shadow_offset).ceil() as i32;
    let w = pw + 2 * pad;
    let h = ph + 2 * pad;

    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
        .expect("Failed to create cairo surface");
    {
        let cr = cairo::Context::new(&surface).expect("Failed to create cairo context");
        let ox = params.outline_px;
        let oy = params.outline_px;

        cr.move_to(ox + params.shadow_offset, oy + params.shadow_offset);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.65);
        pangocairo::functions::show_layout(&cr, &layout);

        cr.move_to(ox, oy);
        pangocairo::functions::layout_path(&cr, &layout);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.set_line_width(params.outline_px * 2.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.95);
        cr.stroke().unwrap();

        cr.move_to(ox, oy);
        let color = &params.color;
        cr.set_source_rgba(
            color.r as f64 / 255.0,
            color.g as f64 / 255.0,
            color.b as f64 / 255.0,
            color.a as f64 / 255.0,
        );
        pangocairo::functions::show_layout(&cr, &layout);
    }
    surface.flush();

    let stride = surface.stride() as usize;
    let data = surface.data().expect("Failed to get cairo surface data").to_vec();
    (data, w, h, stride)
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
    pub screen_height: f32,
    pub intensity_index: u32,
    /// When true, skip row collision detection and let danmaku overlap freely.
    pub allow_overlay: bool,
    overlay_scroll_hint: usize,
    overlay_top_hint: usize,
    overlay_bottom_hint: usize,

    texture_tx: Sender<TextureReady>,
    texture_rx: Receiver<TextureReady>,
    pending_scroll: Vec<PendingScrollRow>,
    next_pending_id: u64,
}

impl Default for DanmakwRenderer {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl DanmakwRenderer {
    pub fn new(scale_factor: f64) -> Self {
        let scroll_max_rows = 25;
        let top_center_max_rows = 10;
        let bottom_center_max_rows = 10;
        let font_size = 24.0_f64;
        let font_px_logical = font_size * (96.0 / 72.0);
        let spacing_factor = 1.5_f32;
        let line_height = font_px_logical as f32 * spacing_factor;
        let spacing = font_px_logical as f32;
        let top_padding = 10.0;

        let top_center_row_occupied = vec![false; top_center_max_rows];
        let bottom_center_row_occupied = vec![false; bottom_center_max_rows];

        let (texture_tx, texture_rx) = flume::unbounded();

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
            screen_height: 0.0,
            intensity_index: 1,
            allow_overlay: false,
            overlay_scroll_hint: 0,
            overlay_top_hint: 0,
            overlay_bottom_hint: 0,
            texture_tx,
            texture_rx,
            pending_scroll: Vec::new(),
            next_pending_id: 0,
        }
    }

    pub fn recompute_max_rows(&mut self) {
        if self.screen_height <= 0.0 || self.line_height <= 0.0 {
            return;
        }

        let total_rows =
            ((self.screen_height - self.top_padding) / self.line_height) as usize;
        let total_rows = total_rows.max(1);

        let scroll = if self.allow_overlay {
            total_rows
        } else {
            let fraction = match self.intensity_index {
                0 => 0.25_f32,
                1 => 0.5,
                _ => 1.0,
            };
            ((total_rows as f32 * fraction) as usize).max(1)
        };

        let center = (scroll / 5).max(1);

        self.scroll_max_rows = scroll;
        self.top_center_max_rows = center;
        self.bottom_center_max_rows = center;
        self.top_center_row_occupied.resize(center, false);
        self.bottom_center_row_occupied.resize(center, false);
    }

    fn spawn_texture(
        &mut self,
        params: TextureRenderParams,
        ready: impl FnOnce(Vec<u8>, i32, i32, usize) -> TextureReady + Send + 'static,
    ) {
        let tx = self.texture_tx.clone();
        spawn_tokio_blocking(move || {
            let (bytes, w, h, stride) = render_texture_raw(params);
            let _ = tx.send(ready(bytes, w, h, stride));
        });
    }

    fn add_scroll_danmaku(
        &mut self,
        params: TextureRenderParams,
        text_width: f32,
        width: f32,
        danmaku: Danmaku,
    ) {
        let velocity_x = -(width + text_width) / SCROLL_DURATION_MS * self.speed_factor as f32;

        let target_row = if self.allow_overlay {
            let row = self.overlay_scroll_hint % self.scroll_max_rows;
            self.overlay_scroll_hint = self.overlay_scroll_hint.wrapping_add(1);
            row
        } else {
            let v = velocity_x.abs();
            let reach_edge_time = width / v;
            let mut found_row: Option<usize> = None;

            for target_row in 0..self.scroll_max_rows {
                let last_in_row = self
                    .scroll_danmaku
                    .iter()
                    .filter(|d| d.row == target_row)
                    .map(|d| (d.x, d.width, d.velocity_x))
                    .chain(
                        self.pending_scroll
                            .iter()
                            .filter(|p| p.row == target_row)
                            .map(|p| (p.x, p.width, p.velocity_x)),
                    )
                    .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

                let Some((last_x, last_width, last_vel)) = last_in_row else {
                    found_row = Some(target_row);
                    break;
                };

                let leave_time = (last_x + last_width + self.spacing) / last_vel.abs();

                if leave_time < reach_edge_time && width > last_width + self.spacing + last_x {
                    found_row = Some(target_row);
                    break;
                }
            }

            let Some(row) = found_row else {
                return;
            };
            row
        };

        let id = self.next_pending_id;
        self.next_pending_id = self.next_pending_id.wrapping_add(1);

        let origin_offset = params.outline_px as f32 / self.scale_factor as f32;

        self.pending_scroll.push(PendingScrollRow {
            id,
            row: target_row,
            x: width,
            velocity_x,
            width: text_width,
        });

        self.spawn_texture(params, move |bytes, w, h, stride| TextureReady::Scroll {
            id,
            bytes,
            w,
            h,
            stride,
            danmaku,
            origin_offset,
            velocity_x,
            width: text_width,
            row: target_row,
        });
    }

    fn add_topcenter_danmaku(
        &mut self,
        params: TextureRenderParams,
        text_width: f32,
        danmaku: Danmaku,
    ) {
        let target_row = if self.allow_overlay {
            let row = self.overlay_top_hint % self.top_center_max_rows;
            self.overlay_top_hint = self.overlay_top_hint.wrapping_add(1);
            row
        } else {
            let Some(row) = self
                .top_center_row_occupied
                .iter()
                .position(|&occupied| !occupied)
            else {
                return;
            };
            self.top_center_row_occupied[row] = true;
            row
        };

        let origin_offset = params.outline_px as f32 / self.scale_factor as f32;

        self.spawn_texture(params, move |bytes, w, h, stride| TextureReady::TopCenter {
            bytes,
            w,
            h,
            stride,
            danmaku,
            origin_offset,
            width: text_width,
            row: target_row,
        });
    }

    fn add_bottomcenter_danmaku(
        &mut self,
        params: TextureRenderParams,
        text_width: f32,
        danmaku: Danmaku,
    ) {
        let target_row = if self.allow_overlay {
            let row = self.overlay_bottom_hint % self.bottom_center_max_rows;
            self.overlay_bottom_hint = self.overlay_bottom_hint.wrapping_add(1);
            row
        } else {
            let Some(row) = self
                .bottom_center_row_occupied
                .iter()
                .position(|&occupied| !occupied)
            else {
                return;
            };
            self.bottom_center_row_occupied[row] = true;
            row
        };

        let origin_offset = params.outline_px as f32 / self.scale_factor as f32;

        self.spawn_texture(params, move |bytes, w, h, stride| TextureReady::BottomCenter {
            bytes,
            w,
            h,
            stride,
            danmaku,
            origin_offset,
            width: text_width,
            row: target_row,
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
        self.pending_scroll.clear();
        while self.texture_rx.try_recv().is_ok() {}

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

        self.apply_ready_textures();

        if delta_time.abs() > RESET_DELTA_MS {
            self.danmaku_queue.reset_time(time_milis);
            return;
        }

        let mut danmaku_queue = std::mem::take(&mut self.danmaku_queue);
        for next_danmaku in danmaku_queue.pop_to_time_iter(time_milis) {
            self.add_danmaku(context, screen_width, next_danmaku.clone());
        }
        self.danmaku_queue = danmaku_queue;

        for text in self.scroll_danmaku.iter_mut() {
            text.x += text.velocity_x * delta_time * self.speed_factor as f32;
        }
        for pending in self.pending_scroll.iter_mut() {
            pending.x += pending.velocity_x * delta_time * self.speed_factor as f32;
        }

        self.scroll_danmaku.retain(|text| text.x + text.width > 0.0);
        self.pending_scroll.retain(|p| p.x + p.width > 0.0);

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

    fn apply_ready_textures(&mut self) {
        while let Ok(ready) = self.texture_rx.try_recv() {
            match ready {
                TextureReady::Scroll {
                    id,
                    bytes,
                    w,
                    h,
                    stride,
                    danmaku,
                    origin_offset,
                    velocity_x,
                    width,
                    row,
                } => {
                    let Some(idx) = self.pending_scroll.iter().position(|p| p.id == id) else {
                        continue;
                    };
                    let current_x = self.pending_scroll[idx].x;
                    self.pending_scroll.swap_remove(idx);

                    if current_x + width <= 0.0 {
                        continue;
                    }

                    let texture = gdk::MemoryTexture::new(
                        w,
                        h,
                        gdk::MemoryFormat::B8g8r8a8Premultiplied,
                        &glib::Bytes::from_owned(bytes),
                        stride,
                    );
                    self.scroll_danmaku.push(ScrollingDanmaku {
                        danmaku,
                        texture,
                        origin_offset,
                        x: current_x,
                        row,
                        velocity_x,
                        width,
                    });
                }
                TextureReady::TopCenter {
                    bytes,
                    w,
                    h,
                    stride,
                    danmaku,
                    origin_offset,
                    width,
                    row,
                } => {
                    let texture = gdk::MemoryTexture::new(
                        w,
                        h,
                        gdk::MemoryFormat::B8g8r8a8Premultiplied,
                        &glib::Bytes::from_owned(bytes),
                        stride,
                    );
                    self.top_center_danmaku.push(CenterDanmaku {
                        danmaku,
                        texture,
                        origin_offset,
                        width,
                        row,
                        remaining_time: CENTER_DURATION_MS,
                    });
                }
                TextureReady::BottomCenter {
                    bytes,
                    w,
                    h,
                    stride,
                    danmaku,
                    origin_offset,
                    width,
                    row,
                } => {
                    let texture = gdk::MemoryTexture::new(
                        w,
                        h,
                        gdk::MemoryFormat::B8g8r8a8Premultiplied,
                        &glib::Bytes::from_owned(bytes),
                        stride,
                    );
                    self.bottom_center_danmaku.push(CenterDanmaku {
                        danmaku,
                        texture,
                        origin_offset,
                        width,
                        row,
                        remaining_time: CENTER_DURATION_MS,
                    });
                }
            }
        }
    }

    pub fn add_danmaku(&mut self, context: &Context, screen_width: f32, danmaku: Danmaku) {
        let raw_dpi = pangocairo::functions::context_get_resolution(context);
        let dpi = if raw_dpi > 0.0 { raw_dpi } else { 96.0 };
        let font_px_device = self.font_size * (dpi / 72.0) * self.scale_factor;
        let font_px_logical = self.font_size * (dpi / 72.0);
        self.line_height = font_px_logical as f32 * self.spacing_factor;
        self.spacing = font_px_logical as f32;

        self.recompute_max_rows();

        let layout = Layout::new(context);
        let mut font_desc = pango::FontDescription::default();
        font_desc.set_absolute_size(font_px_device * pango::SCALE as f64);
        font_desc.set_family(&self.font_name);
        font_desc.set_weight(self.font_weight);
        layout.set_font_description(Some(&font_desc));
        layout.set_text(&danmaku.content);

        let text_width = layout.pixel_size().0 as f32 / self.scale_factor as f32;
        drop(layout);

        let params = TextureRenderParams {
            text: danmaku.content.clone(),
            font_family: self.font_name.clone(),
            font_weight: self.font_weight,
            font_px_device,
            dpi,
            color: danmaku.color,
            outline_px: self.outline_px,
            shadow_offset: self.shadow_offset,
        };

        match danmaku.mode {
            DanmakuMode::Scroll => {
                self.add_scroll_danmaku(params, text_width, screen_width, danmaku);
            }
            DanmakuMode::TopCenter => {
                self.add_topcenter_danmaku(params, text_width, danmaku);
            }
            DanmakuMode::BottomCenter => {
                self.add_bottomcenter_danmaku(params, text_width, danmaku);
            }
        }
    }

    pub fn clear_danmaku(&mut self) {
        self.scroll_danmaku.clear();
        self.top_center_danmaku.clear();
        self.bottom_center_danmaku.clear();
        self.top_center_row_occupied.fill(false);
        self.bottom_center_row_occupied.fill(false);
        self.pending_scroll.clear();
        while self.texture_rx.try_recv().is_ok() {}
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
        self.intensity_index = index;
        self.allow_overlay = index == 3;
        self.recompute_max_rows();
    }
}
