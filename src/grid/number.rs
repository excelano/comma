// The number down the side of the table, and the handle for the row it counts.
//
// It is a widget of its own rather than a bare label because a handle has to
// know which row it is, and it has to know it twice over: the file's row, which
// is what an operation is addressed by, and the view's position, which is where
// the keyboard goes. Widgets are recycled as the table scrolls, so a number that
// only had text in it would have nothing to say by the time it was clicked.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::Cell as Value;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::point_in_view;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Number {
        pub label: gtk::Label,
        /// Where the row sits in the view, which is what moving the keyboard
        /// counts in.
        pub position: Value<u32>,
        /// Where the row sits in the file, which is what the number shows.
        pub index: Value<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Number {
        const NAME: &'static str = "CommaNumber";
        type Type = super::Number;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Number {
        fn constructed(&self) {
            self.parent_constructed();

            self.label.set_xalign(1.0);
            self.label.set_hexpand(true);
            self.label.set_css_classes(&["dim-label", "numeric"]);
            self.label.set_parent(&*self.obj());
        }

        fn dispose(&self) {
            self.label.unparent();
        }
    }

    impl WidgetImpl for Number {}
}

glib::wrapper! {
    pub struct Number(ObjectSubclass<imp::Number>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Number {
    pub(super) fn new() -> Self {
        let number: Self = glib::Object::new();
        number.set_accessible_role(gtk::AccessibleRole::RowHeader);
        number.connect_gestures();
        number
    }

    /// Where this number's row sits in the view, which is what the gutter is
    /// told about when the keyboard moves.
    pub(super) fn position(&self) -> u32 {
        self.imp().position.get()
    }

    /// Sizes the number to the widest one the file can show, so the gutter does
    /// not grow as you scroll into four-digit territory.
    pub(super) fn set_digits(&self, digits: i32) {
        self.imp().label.set_width_chars(digits);
    }

    /// Lights the number, or stops. This says where the keyboard is, and says
    /// nothing about selection: there is no such thing here.
    pub(super) fn set_current(&self, current: bool) {
        if current {
            self.add_css_class("current");
        } else {
            self.remove_css_class("current");
        }
    }

    /// Says which row this now counts, and how tall that row is. Files are
    /// numbered from one everywhere a person will read the number, including in
    /// every other tool that opens them.
    ///
    /// The height is asked for rather than taken from the row beside it: that
    /// row is in another view, and a number an inch short of its row would put
    /// every number below it beside the wrong one.
    pub(super) fn show_row(&self, position: u32, index: usize, lines: usize) {
        let imp = self.imp();
        imp.position.set(position);
        imp.index.set(index);
        imp.label
            .set_size_request(-1, super::height_for_lines(&imp.label, lines));

        let number = (index + 1).to_string();
        imp.label.set_text(&number);
        // The label is inside this widget rather than being it, so the number
        // would otherwise be something a screen reader could see but not say.
        // The role says it is a row header; there is no word to add to that.
        self.update_property(&[gtk::accessible::Property::Label(&number)]);
    }

    /// What a press on the number does: puts the keyboard at the start of that
    /// row, and on the other button asks what can be done to it. Both begin the
    /// same way, because a menu whose items act on the cursor has to move the
    /// cursor first.
    fn connect_gestures(&self) {
        let clicks = gtk::GestureClick::new();
        clicks.set_button(gdk::BUTTON_PRIMARY);
        clicks.connect_pressed(glib::clone!(
            #[weak(rename_to = number)]
            self,
            move |_, _, _, _| number.go_to_row()
        ));
        self.add_controller(clicks);

        let menu = gtk::GestureClick::new();
        menu.set_button(gdk::BUTTON_SECONDARY);
        menu.connect_pressed(glib::clone!(
            #[weak(rename_to = number)]
            self,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                number.go_to_row();

                if let Some((x, y)) = point_in_view(number.upcast_ref(), x, y) {
                    number
                        .activate_action("win.row-menu", Some(&(x, y).to_variant()))
                        .unwrap_or_default();
                }
            }
        ));
        self.add_controller(menu);
    }

    /// The start of the row, rather than the column the cursor was in. On a wide
    /// file that column is somewhere off the side of the screen, and landing
    /// there after clicking something on the left edge reads as the table
    /// jumping away.
    fn go_to_row(&self) {
        let position = self.imp().position.get();
        self.activate_action("win.go-to", Some(&(position, 0u32).to_variant()))
            .unwrap_or_default();
    }
}
