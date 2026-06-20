use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use adw::subclass::prelude::*;
use gtk::{CompositeTemplate, glib};
use mutsumi::{Color, DanmakuMode, MutsumiPlayer, PlayParams, PlaySource};

use crate::PlayList;
use crate::danmaku::{
    LiveDanmaku, get_douyu_stream_url, parse_bilibili_live_room_id, parse_douyu_room_id,
    spawn_bilibili_live_danmaku, spawn_douyu_live_danmaku,
};

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::prelude::*;
    use glib::subclass::InitializingObject;

    use crate::status::PlaceHolderStatus;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/mutsumi-live/ui/window.ui")]
    pub struct MutsumiPlayerWindow {
        #[template_child]
        pub player: TemplateChild<MutsumiPlayer>,
        pub playlist: OnceCell<PlayList>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MutsumiPlayerWindow {
        const NAME: &'static str = "MutsumiPlayerWindow";
        type Type = super::MutsumiPlayerWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            MutsumiPlayer::ensure_type();
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MutsumiPlayerWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let playlist = PlayList::new();
            self.player.playlist_bin().set_child(Some(&playlist));
            self.player.playlist_stack_page().set_visible(true);

            let obj = self.obj();

            let place_holder_status = PlaceHolderStatus::new();
            place_holder_status.connect_button_clicked(glib::clone!(
                #[weak]
                obj,
                move || {
                    obj.player().open_playlist();
                }
            ));
            self.player.overlay_status().set_child(Some(&place_holder_status));

            self.playlist.set(playlist).unwrap();
        }

        fn dispose(&self) {
            if let Some(stop) = self.danmaku_stop.take() {
                stop.store(false, Ordering::Relaxed);
            }
        }
    }

    impl WidgetImpl for MutsumiPlayerWindow {}
    impl WindowImpl for MutsumiPlayerWindow {}
    impl ApplicationWindowImpl for MutsumiPlayerWindow {}
    impl AdwApplicationWindowImpl for MutsumiPlayerWindow {}
}

glib::wrapper! {
    pub struct MutsumiPlayerWindow(ObjectSubclass<imp::MutsumiPlayerWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
                    gtk::Native, gtk::Root, gtk::ShortcutManager,
                    gtk::gio::ActionGroup, gtk::gio::ActionMap;
}

impl MutsumiPlayerWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn player(&self) -> MutsumiPlayer {
        self.imp().player.get()
    }
}
