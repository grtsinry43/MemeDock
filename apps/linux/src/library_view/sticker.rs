use gtk4::glib::{self, Properties};
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use memedock_domain::asset::ImageFormat;
use std::cell::{Cell, RefCell};
use std::path::Path;

glib::wrapper! {
    pub struct StickerObject(ObjectSubclass<imp::StickerObject>);
}

impl StickerObject {
    pub fn new(
        identity: &str,
        title: &str,
        animated: bool,
        starred: bool,
        thumbnail: Option<&Path>,
        entity_generation: i64,
        entity_revision: i64,
    ) -> Self {
        glib::Object::builder::<Self>()
            .property("identity", identity)
            .property("title", title)
            .property("animated", animated)
            .property("starred", starred)
            .property(
                "thumbnail",
                thumbnail.and_then(|path| path.to_str()).unwrap_or(""),
            )
            .property("entity_generation", entity_generation)
            .property("entity_revision", entity_revision)
            .build()
    }

    pub fn source_format(&self) -> ImageFormat {
        match self.source_mime().as_str() {
            "image/jpeg" => ImageFormat::Jpeg,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::WebP,
            _ => ImageFormat::Png,
        }
    }

    pub fn watch_picture(&self, picture: &gtk4::Picture) {
        self.imp().picture.set(Some(picture));
    }

    pub fn clear_picture(&self) {
        self.imp().picture.set(None);
    }

    pub fn picture(&self) -> Option<gtk4::Picture> {
        self.imp().picture.upgrade()
    }
}

mod imp {
    use super::*;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::StickerObject)]
    pub struct StickerObject {
        #[property(get, set)]
        identity: RefCell<String>,
        #[property(get, set)]
        title: RefCell<String>,
        #[property(get, set)]
        animated: Cell<bool>,
        #[property(get, set)]
        starred: Cell<bool>,
        #[property(get, set)]
        thumbnail: RefCell<String>,
        #[property(get, set)]
        generation: Cell<u64>,
        #[property(get, set)]
        entity_generation: Cell<i64>,
        #[property(get, set)]
        entity_revision: Cell<i64>,
        #[property(get, set)]
        source_mime: RefCell<String>,
        pub picture: glib::WeakRef<gtk4::Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StickerObject {
        const NAME: &'static str = "MemeDockSticker";
        type Type = super::StickerObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for StickerObject {}
}
