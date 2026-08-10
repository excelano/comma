// The list model the column view reads.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use comma::document::Document;

use super::Row;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct RowModel {
        pub document: RefCell<Option<Rc<RefCell<Document>>>>,
        /// Whether the first record of the file is its column titles rather
        /// than data. The document does not know or care; this is a view of it.
        pub header: Cell<bool>,
    }

    impl RowModel {
        /// The document row the first row of the grid comes from.
        pub fn first_row(&self) -> usize {
            usize::from(self.header.get())
        }

        pub fn rows(&self) -> usize {
            match self.document.borrow().as_ref() {
                Some(document) => document.borrow().row_count(),
                None => 0,
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RowModel {
        const NAME: &'static str = "CommaRowModel";
        type Type = super::RowModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for RowModel {}

    impl ListModelImpl for RowModel {
        fn item_type(&self) -> glib::Type {
            Row::static_type()
        }

        fn n_items(&self) -> u32 {
            self.rows().saturating_sub(self.first_row()) as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let document = self.document.borrow().clone()?;
            let row = position as usize + self.first_row();
            if row >= document.borrow().row_count() {
                return None;
            }
            Some(Row::new(document.clone(), row).upcast())
        }
    }
}

glib::wrapper! {
    pub struct RowModel(ObjectSubclass<imp::RowModel>) @implements gio::ListModel;
}

impl Default for RowModel {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl RowModel {
    /// Puts a document behind the model, replacing whatever was there.
    pub fn set_document(&self, document: Document) {
        let removed = self.n_items();

        self.imp()
            .document
            .replace(Some(Rc::new(RefCell::new(document))));

        self.items_changed(0, removed, self.n_items());
    }

    pub fn document(&self) -> Option<Rc<RefCell<Document>>> {
        self.imp().document.borrow().clone()
    }

    pub fn header(&self) -> bool {
        self.imp().header.get()
    }

    /// Takes the first record out of the body, or puts it back. Nothing is
    /// hidden that the file does not still hold: the row is a title now, and
    /// the numbers on the remaining rows still say where in the file they are.
    pub fn set_header(&self, header: bool) {
        if self.header() == header {
            return;
        }

        let removed = self.n_items();
        self.imp().header.set(header);
        self.items_changed(0, removed, self.n_items());
    }
}
