// The main window. For now it shows an empty state and nothing else.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::application::CommaApplication;
use crate::config::APP_ID;

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/com/excelano/Comma/gtk/window.ui")]
    pub struct CommaWindow {
        pub settings: gio::Settings,
    }

    impl Default for CommaWindow {
        fn default() -> Self {
            Self {
                settings: gio::Settings::new(APP_ID),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CommaWindow {
        const NAME: &'static str = "CommaWindow";
        type Type = super::CommaWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CommaWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().bind_window_state();
        }
    }

    impl WidgetImpl for CommaWindow {}
    impl WindowImpl for CommaWindow {}
    impl ApplicationWindowImpl for CommaWindow {}
    impl AdwApplicationWindowImpl for CommaWindow {}
}

glib::wrapper! {
    pub struct CommaWindow(ObjectSubclass<imp::CommaWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CommaWindow {
    pub fn new(application: &CommaApplication) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    /// Window geometry survives a restart, as GNOME apps are expected to do.
    fn bind_window_state(&self) {
        let settings = &self.imp().settings;
        settings.bind("window-width", self, "default-width").build();
        settings
            .bind("window-height", self, "default-height")
            .build();
        settings.bind("window-maximized", self, "maximized").build();
    }
}
