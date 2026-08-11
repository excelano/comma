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
        /// How many rows the view was last told there were. A list model has to
        /// say how many items came and went, and the document it reads has
        /// already changed by the time it is asked.
        pub reported: Cell<u32>,
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
        self.imp()
            .document
            .replace(Some(Rc::new(RefCell::new(document))));
        self.resync();
    }

    /// Says that the document has changed in ways the view cannot be told about
    /// row by row, because rows or columns came or went.
    pub fn reload(&self) {
        self.resync();
    }

    /// Tells the view that every row it knows about is gone and these are the
    /// rows there are now. Redrawing all of it is the honest answer when what
    /// moved is which rows there are.
    fn resync(&self) {
        let removed = self.imp().reported.replace(self.n_items());
        self.items_changed(0, removed, self.n_items());
    }

    pub fn document(&self) -> Option<Rc<RefCell<Document>>> {
        self.imp().document.borrow().clone()
    }

    pub fn header(&self) -> bool {
        self.imp().header.get()
    }

    /// Says that records were spliced: `gone` of them from `at`, replaced by
    /// `come`. Everything below the splice keeps its widgets and the grid keeps
    /// the place it was scrolled to, which is what tells a row being inserted
    /// from the file being opened again.
    ///
    /// The caller has already settled that the splice does not reach the header
    /// record, which moves more of the grid than this can describe.
    pub fn rows_changed(&self, at: usize, gone: usize, come: usize) {
        let position = at.saturating_sub(self.imp().first_row()) as u32;
        self.imp().reported.set(self.n_items());
        self.items_changed(position, gone as u32, come as u32);
    }

    /// Says that one record of the document now reads differently, so the view
    /// draws that row again and leaves the rest alone. A record the header
    /// toggle has taken out of the body is not a row here and is ignored.
    pub fn row_changed(&self, row: usize) {
        let Some(position) = row.checked_sub(self.imp().first_row()) else {
            return;
        };
        if (position as u32) < self.n_items() {
            self.items_changed(position as u32, 1, 1);
        }
    }

    /// Takes the first record out of the body, or puts it back. Nothing is
    /// hidden that the file does not still hold: the row is a title now, and
    /// the numbers on the remaining rows still say where in the file they are.
    pub fn set_header(&self, header: bool) {
        if self.header() == header {
            return;
        }

        self.imp().header.set(header);
        self.resync();
    }
}
