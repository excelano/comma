// The row numbers, as a view of their own.
//
// They used to be column zero of the table, which put them inside the part of it
// that scrolls sideways: the column headings stayed where they were and the
// numbers naming the rows slid away under them. GtkColumnView cannot pin a
// column, so the numbers are a second view beside the first, sharing the table's
// vertical adjustment and the model behind it. Row n of one is row n of the
// other by identity rather than by arithmetic, and the two move together because
// one adjustment moves both.
//
// It is a column view rather than a list view for the two things that line the
// halves up. It draws its own header row, so the first number sits beside the
// first row without a spacer kept level with a widget GTK owns; and its rows are
// the same CSS nodes as the table's, so the padding and the separators come from
// the rules already styling those.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::number::Number;
use super::{Records, as_cell, visit};

/// The row numbers beside the table.
///
/// What it holds is what a number cannot be asked, because the widgets are
/// handed from row to row as the view scrolls and a number that has just been
/// handed a row has to be told both of these again.
#[derive(Debug, Default)]
pub struct Gutter {
    view: OnceCell<gtk::ColumnView>,
    /// How wide the numbers are, in digits. Sized to the widest number the file
    /// can show rather than to the ones on screen, so the gutter does not
    /// change width as you scroll.
    digits: Rc<Cell<i32>>,
    /// Which row of the view the keyboard is on, which is the one lit up.
    /// Nothing is, while the keyboard is somewhere other than the table.
    current: Rc<Cell<Option<u32>>>,
}

impl Gutter {
    /// Takes over a column view and puts the row numbers in it: one column, no
    /// title, nothing to sort by, and the table's own model, so that whatever
    /// the table is showing and in whatever order, this is showing the same.
    pub fn attach(&self, view: &gtk::ColumnView, model: &impl IsA<gtk::SelectionModel>) {
        let records = Records::new(model);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(glib::clone!(
            #[strong]
            records,
            move |_, item| {
                let item = as_cell(item);
                item.set_child(Some(&Number::new(records.clone(), item)));
            }
        ));

        let digits = self.digits.clone();
        let current = self.current.clone();
        factory.connect_bind(move |_, item| {
            let Some(number) = as_cell(item).child().and_downcast::<Number>() else {
                return;
            };
            number.set_digits(digits.get());
            number.show_row();
            number.set_current(number.position() == current.get());
        });

        view.append_column(
            &gtk::ColumnViewColumn::builder()
                .factory(&factory)
                .resizable(false)
                .build(),
        );
        view.set_model(Some(model));

        self.view
            .set(view.clone())
            .expect("the gutter is attached to one view, once");
    }

    /// Says how wide the numbers have to be, for a file of however many rows it
    /// has now. A file crossing a thousand rows widens them where they are rather
    /// than sending the table off to be built again.
    pub fn set_digits(&self, digits: i32) {
        if self.digits.replace(digits) == digits {
            return;
        }
        self.each_number(&mut |number| number.set_digits(digits));
    }

    /// Says which row of the view the keyboard is on, so that the number beside
    /// it is lit along with the row itself.
    ///
    /// The table lights its own row from `:focus-within`, which it can do
    /// because the cell with the keyboard in it is inside that row. Nothing here
    /// is inside anything, so the numbers are told.
    pub fn set_current(&self, position: Option<u32>) {
        if self.current.replace(position) == position {
            return;
        }
        self.each_number(&mut |number| number.set_current(number.position() == position));
    }

    /// Says the numbers again, for when the rows have moved under them.
    ///
    /// A row put in above another moves it without binding it again, which is
    /// right for the value it holds and wrong for the number beside it: the
    /// value belongs to the row and the number belongs to the place. The rows on
    /// screen are asked where they are now.
    pub fn renumber(&self) {
        let current = self.current.get();
        self.each_number(&mut |number| {
            number.show_row();
            number.set_current(number.position() == current);
        });
    }

    /// Every number on screen. The rest of the file has none yet.
    fn each_number(&self, act: &mut dyn FnMut(&Number)) {
        let Some(view) = self.view.get() else {
            return;
        };
        visit(view.upcast_ref(), &mut |number: &Number| {
            act(number);
            glib::ControlFlow::Continue
        });
    }
}
