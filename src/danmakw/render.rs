use crate::DanmakwRenderer;
use gtk::prelude::*;

pub trait DanmakwSnapshotExt {
    fn render_danmakw(&self, renderer: &mut DanmakwRenderer, width: f32, height: f32);
}

impl DanmakwSnapshotExt for gtk::Snapshot {
    fn render_danmakw(&self, renderer: &mut DanmakwRenderer, width: f32, height: f32) {
        if renderer.screen_height != height {
            renderer.screen_height = height;
            renderer.recompute_max_rows();
        }
        let scale = renderer.scale_factor as f32;

        for sd in renderer.scroll_danmaku.iter() {
            let off = sd.origin_offset;
            let bounds = gtk::graphene::Rect::new(
                sd.x - off,
                renderer.scrolled_top_y(sd.row) - off,
                sd.texture.width() as f32 / scale,
                sd.texture.height() as f32 / scale,
            );
            self.append_texture(&sd.texture, &bounds);
        }

        for cd in renderer.top_center_danmaku.iter() {
            let off = cd.origin_offset;
            let x = (width - cd.width) / 2.0;
            let y = renderer.top_center_y(cd.row);
            let bounds = gtk::graphene::Rect::new(
                x - off,
                y - off,
                cd.texture.width() as f32 / scale,
                cd.texture.height() as f32 / scale,
            );
            self.append_texture(&cd.texture, &bounds);
        }

        for cd in renderer.bottom_center_danmaku.iter() {
            let off = cd.origin_offset;
            let x = (width - cd.width) / 2.0;
            let y = renderer.bottom_center_y(cd.row, height);
            let bounds = gtk::graphene::Rect::new(
                x - off,
                y - off,
                cd.texture.width() as f32 / scale,
                cd.texture.height() as f32 / scale,
            );
            self.append_texture(&cd.texture, &bounds);
        }
    }
}
