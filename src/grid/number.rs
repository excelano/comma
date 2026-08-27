// The number down the side of the table, and the handle for the row it counts.
//
// It is a widget of its own rather than a bare label because a handle has to
// know which row it is: a press on it moves the keyboard there and the other
// button asks what can be done to it, and a number that only had text in it
// would have nothing to say by the time it was clicked.
//
// It is told where it is rather than asking, which is the other way round from
// how this worked while the numbers were a view of their own. There is no view
// now: the gutter puts a number against each row the table has drawn and says
// which row that is, every time the grid is laid out, so a number cannot be left
// naming a row that has moved.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::Cell as Value;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::point_in;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Number {
        pub label: gtk::Label,
        /// Which row of the view this is drawn against, as of the last time the
        /// grid was laid out. Nothing here works it out; the gutter says.
        pub position: Value<Option<u32>>,
        /// Which record of the file that row is showing, which is what the
        /// number says. Kept apart from the position so that the text is only
        /// written when it changes.
        pub row: Value<Option<usize>>,
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

            self.obj().add_css_class("number");
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

    /// Where this number's row sits in the view, as of the last time the grid was
    /// laid out.
    pub(super) fn position(&self) -> Option<u32> {
        self.imp().position.get()
    }

    /// Sizes the number to the widest one the file can show, so the gutter does
    /// not grow as you scroll into four-digit territory.
    pub(super) fn set_digits(&self, digits: i32) {
        self.imp().label.set_width_chars(digits);
    }

    /// Says which row this counts, and whether the keyboard is on it.
    ///
    /// Files are numbered from one everywhere a person will read the number,
    /// including in every other tool that opens them. How tall the number is is
    /// not its business any more: the gutter gives it the height of the row it
    /// is drawn against, which is the row's own height rather than a second
    /// calculation that has to agree with it.
    pub(super) fn show_row(&self, position: u32, row: usize, current: bool) {
        let imp = self.imp();
        imp.position.set(Some(position));
        if imp.row.replace(Some(row)) != Some(row) {
            let number = (row + 1).to_string();
            imp.label.set_text(&number);
            // The label is inside this widget rather than being it, so the
            // number would otherwise be something a screen reader could see but
            // not say. The role says it is a row header; there is nothing to add.
            self.update_property(&[gtk::accessible::Property::Label(&number)]);
        }

        if current {
            self.add_css_class("current");
        } else {
            self.remove_css_class("current");
        }
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

                let at = point_in(number.upcast_ref(), super::Gutter::static_type(), x, y);
                if let Some((x, y)) = at {
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
        let Some(position) = self.position() else {
            return;
        };
        self.activate_action("win.go-to", Some(&(position, 0u32).to_variant()))
            .unwrap_or_default();
    }
}
