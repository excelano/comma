// The GApplication subclass: owns the app-wide actions and the main window.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::config::{APP_ID, VERSION};
use crate::window::CommaWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct CommaApplication;

    #[glib::object_subclass]
    impl ObjectSubclass for CommaApplication {
        const NAME: &'static str = "CommaApplication";
        type Type = super::CommaApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for CommaApplication {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_actions();
        }
    }

    impl ApplicationImpl for CommaApplication {
        fn activate(&self) {
            let application = self.obj();
            let window = application
                .active_window()
                .unwrap_or_else(|| CommaWindow::new(&application).upcast());
            window.present();
        }
    }

    impl GtkApplicationImpl for CommaApplication {}
    impl AdwApplicationImpl for CommaApplication {}
}

glib::wrapper! {
    pub struct CommaApplication(ObjectSubclass<imp::CommaApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl CommaApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/com/excelano/Comma")
            .build()
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|application: &Self, _, _| application.quit())
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|application: &Self, _, _| application.show_about())
            .build();
        self.add_action_entries([quit, about]);

        self.set_accels_for_action("app.quit", &["<primary>q"]);
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Comma")
            .application_icon(APP_ID)
            .version(VERSION)
            .developer_name("David M. Anderson")
            .developers(vec!["David M. Anderson"])
            .copyright("© 2026 David M. Anderson")
            .license_type(gtk::License::MitX11)
            .website("https://github.com/excelano/comma")
            .issue_url("https://github.com/excelano/comma/issues")
            .build();

        about.present(self.active_window().as_ref());
    }
}
