// One row of the grid, as an object the view can hold on to.
//
// A Row is a position and a share in the document, not a copy of anything. Two
// Rows for the same position are interchangeable, which is what lets the model
// build them on demand and throw them away when the view scrolls past.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::subclass::prelude::*;

use comma::document::Document;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Row {
        pub document: RefCell<Option<Rc<RefCell<Document>>>>,
        pub index: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "CommaRow";
        type Type = super::Row;
    }

    impl ObjectImpl for Row {}
}

glib::wrapper! {
    pub struct Row(ObjectSubclass<imp::Row>);
}

impl Row {
    pub(super) fn new(document: Rc<RefCell<Document>>, index: usize) -> Self {
        let row: Self = glib::Object::new();
        row.imp().document.replace(Some(document));
        row.imp().index.set(index);
        row
    }

    /// Which record of the file this is, counting from zero, which is how the
    /// document is addressed.
    pub(super) fn index(&self) -> usize {
        self.imp().index.get()
    }

    /// What the gutter shows. Files are counted from one everywhere a person
    /// will read the number, including in every other tool that opens them.
    pub fn number(&self) -> usize {
        self.imp().index.get() + 1
    }

    pub fn value(&self, column: usize) -> String {
        let imp = self.imp();
        match imp.document.borrow().as_ref() {
            Some(document) => document.borrow().value(imp.index.get(), column).to_owned(),
            None => String::new(),
        }
    }
}
