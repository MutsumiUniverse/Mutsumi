use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::PlaylistItem)]
    pub struct PlaylistItem {
        #[property(get, set)]
        pub filename: RefCell<String>,
        #[property(get, set)]
        pub title: RefCell<String>,
        #[property(get, set)]
        pub current: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistItem {
        const NAME: &'static str = "MutsumiPlaylistItem";
        type Type = super::PlaylistItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlaylistItem {}
}

glib::wrapper! {
    /// A single entry of the player's playlist store.
    pub struct PlaylistItem(ObjectSubclass<imp::PlaylistItem>);
}

impl Default for PlaylistItem {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaylistItem {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn with_values(filename: &str, title: &str, current: bool) -> Self {
        glib::Object::builder()
            .property("filename", filename)
            .property("title", title)
            .property("current", current)
            .build()
    }
}
