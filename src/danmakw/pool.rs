use crate::LayoutExt;
use gtk::{gdk::FrameClock, glib, prelude::*, subclass::prelude::*};
use std::cell::RefCell;

pub struct Danmaku {
    layout: pango::Layout,
    x: f32,
    y: f32,
}

impl Danmaku {
    pub fn context_x(&self, width: f32) -> f32 {
        width - self.x
    }

    pub fn dx_per_millis(&self, width: f32) -> f32 {
        self.layout.dx_per_millis(width)
    }
}

mod imp {
    use crate::DanmakuClock;

    use super::*;
    use gtk::TickCallbackId;
    use pango::FontDescription;

    #[derive(Default)]
    pub struct Danmakw {
        pub danmakus: RefCell<Vec<Danmaku>>,
        pub timer: RefCell<DanmakuClock>,

        pub tick_callback_id: RefCell<Option<TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Danmakw {
        const NAME: &'static str = "Danmakw";
        type Type = super::Danmakw;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Danmakw {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.add_tick_callback(super::Danmakw::cb);
        }
    }

    impl WidgetImpl for Danmakw {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let danmakus = self.danmakus.borrow();

            let width = self.obj().width() as f32;

            for d in danmakus.iter() {
                snapshot.save();
                let x = d.context_x(width);
                snapshot.translate(&gtk::graphene::Point::new(x, d.y));
                snapshot.append_layout(&d.layout, &gtk::gdk::RGBA::WHITE);
                snapshot.restore();
            }
        }
    }

    impl Danmakw {
        pub fn add_danmaku(&self, context: pango::Context, text: &str) {
            let layout = pango::Layout::new(&context);

            let mut font_desc = FontDescription::default();
            font_desc.set_size(32 * pango::SCALE);

            layout.set_font_description(Some(&font_desc));
            layout.set_text(text);

            let width = self.obj().width() as f32;

            self.danmakus.borrow_mut().push(Danmaku {
                layout,
                x: 0.0,
                y: 0.0,
            });
        }
    }
}

glib::wrapper! {
    pub struct Danmakw(ObjectSubclass<imp::Danmakw>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Danmakw {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn add_danmaku(&self, text: &str) {
        self.imp().add_danmaku(self.pango_context(), text);
    }

    pub fn start_rendering(&self) {
        self.imp()
            .tick_callback_id
            .replace(Some(self.add_tick_callback(Self::cb)));
    }

    pub fn stop_rendering(&self) {
        if let Some(id) = self.imp().tick_callback_id.borrow_mut().take() {
            id.remove();
        }
    }

    fn cb(&self, clock: &FrameClock) -> glib::ControlFlow {
        let imp = self.imp();
        let mut danmakus = imp.danmakus.borrow_mut();
        let width = self.width() as f32;

        for d in danmakus.iter_mut() {
            d.x += d.dx_per_millis(width)
        }

        self.queue_draw();
        glib::ControlFlow::Continue
    }
}
