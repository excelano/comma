// The GApplication subclass: owns the app-wide actions and the main window.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::ops::ControlFlow;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gdk;
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
            self.obj().setup_options();
        }
    }

    impl ApplicationImpl for CommaApplication {
        /// The one question that is about Comma rather than about a file, so
        /// it is answered here, before anything registers or a window is
        /// built. Breaking out of the run is what makes `comma --version`
        /// print a line and end rather than open an empty grid.
        fn handle_local_options(&self, options: &glib::VariantDict) -> ControlFlow<glib::ExitCode> {
            if options.contains("version") {
                println!("Comma {VERSION}");
                return ControlFlow::Break(glib::ExitCode::SUCCESS);
            }
            self.parent_handle_local_options(options)
        }

        fn startup(&self) {
            self.parent_startup();
            self.obj().load_styles();
        }

        fn activate(&self) {
            let application = self.obj();
            let window = application
                .active_window()
                .unwrap_or_else(|| CommaWindow::new(&application).upcast());
            window.present();
        }

        /// Files named on the command line, or handed over by the file manager.
        /// A window holds one document, so each file gets one.
        fn open(&self, files: &[gio::File], _hint: &str) {
            let application = self.obj();
            for file in files {
                let window = CommaWindow::new(&application);
                window.present();

                // Reading waits for the next turn of the main loop. A file
                // that will not open says so in a dialog, and a dialog
                // presented in the same turn that asked for the window is
                // never seen.
                let file = file.clone();
                glib::idle_add_local_once(move || window.open_file(&file));
            }
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
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .build()
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|application: &Self, _, _| application.quit())
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|application: &Self, _, _| application.show_about())
            .build();
        // A window holds one document, so a second document needs a second
        // window. This is what open() does for every file it is handed, and
        // until now it was the only thing that could.
        let new_window = gio::ActionEntry::builder("new-window")
            .activate(|application: &Self, _, _| CommaWindow::new(application).present())
            .build();
        self.add_action_entries([quit, about, new_window]);

        self.set_accels_for_action("app.quit", &["<primary>q"]);
        self.set_accels_for_action("app.new-window", &["<primary>n"]);
        // GTK installs this one on every window itself. Comma's close_request
        // asks about an unsaved document before letting it go, so the key
        // inherits that and needs nothing of its own.
        self.set_accels_for_action("window.close", &["<primary>w"]);
        self.set_accels_for_action("win.open", &["<primary>o"]);
        self.set_accels_for_action("win.save", &["<primary>s"]);
        self.set_accels_for_action("win.reload", &["<primary>r"]);
        self.set_accels_for_action("win.save-as", &["<primary><shift>s"]);
        self.set_accels_for_action("win.find", &["<primary>f"]);
        // The target has to say what width of number it is, or GIO reads a bare
        // -1 as the wrong kind and the accelerator never binds. This one means
        // the column the cursor is in.
        self.set_accels_for_action("win.filter-column(int32 -1)", &["<primary><shift>f"]);
        // Both forms, because reaching "?" needs Shift on most layouts and the
        // key that arrives then carries it. Neither fires on every desktop, so
        // the menu item is the way in that always works.
        self.set_accels_for_action(
            "win.shortcuts",
            &[
                "<primary>question",
                "<primary><shift>question",
                "<primary><shift>slash",
            ],
        );
        self.set_accels_for_action("win.undo", &["<primary>z"]);
        self.set_accels_for_action("win.redo", &["<primary><shift>z", "<primary>y"]);
    }

    /// What `--help` and `--version` answer.
    ///
    /// GApplication writes the help itself and does it well, but only about the
    /// options. The two lines it cannot know are the ones a person runs
    /// `--help` to read: that Comma takes files, and what it does with them.
    fn setup_options(&self) {
        self.add_main_option(
            "version",
            glib::Char::from(b'V'),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            &gettext("Show the version and exit"),
            None,
        );

        // Plural, because a file per window is what open() does with them.
        self.set_option_context_parameter_string(Some(&gettext("[FILE…]")));
        self.set_option_context_summary(Some(&gettext(
            "Edit CSV, TSV and other delimited text files as a grid.\nEach file named opens in a window of its own.",
        )));
    }

    /// Comma's own styling, on top of whatever theme the desktop is wearing.
    fn load_styles(&self) {
        let Some(display) = gdk::Display::default() else {
            return;
        };

        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/com/excelano/Comma/style.css");
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
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
