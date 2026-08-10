// The list model the column view reads.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::RefCell;
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
            match self.document.borrow().as_ref() {
                Some(document) => document.borrow().row_count() as u32,
                None => 0,
            }
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let document = self.document.borrow().clone()?;
            let rows = document.borrow().row_count();
            if position as usize >= rows {
                return None;
            }
            Some(Row::new(document.clone(), position as usize).upcast())
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
        let added = document.row_count() as u32;
        let removed = self.n_items();

        self.imp()
            .document
            .replace(Some(Rc::new(RefCell::new(document))));

        self.items_changed(0, removed, added);
    }
}
