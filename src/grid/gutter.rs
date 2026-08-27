// The row numbers, drawn against the rows they name.
//
// They used to be column zero of the table, which put them inside the part of it
// that scrolls sideways: the column headings stayed where they were and the
// numbers naming the rows slid away under them. GtkColumnView cannot pin a
// column, so the numbers left the view.
//
// What they became first was a second view beside it, sharing the table's model
// and its vertical adjustment. That is the shape this replaces, and why is worth
// keeping. A GtkListView does not hold a scroll offset; it holds the row it was
// showing and how far into it, and works the offset out from those. Handed an
// offset it works back the other way, using the heights it has measured — which
// are not the heights the other view has measured, because the two have not had
// the same rows through them. So one number put the two views on different rows,
// each carried its answer forward rather than deriving it afresh, and every
// wheel click left a little more between them than the last one did. Two hundred
// of them was a number naming a row four rows from the one it was drawn beside.
// Four ways of driving one view from the other were tried and each failed the
// same way: a list view owns where it is and re-derives it whenever it is
// pushed, so it cannot be told.
//
// So there is no second view to keep level. This widget holds the table and puts
// a number against each row the table has just drawn, in the same breath: it
// allocates the table first, then reads the rows out of it and gives each number
// that row's place and that row's height. Nothing is estimated and nothing is
// shared, so there is nothing to drift. It is also why two scrolled windows can
// no longer write to one adjustment, which is what used to stop a file with line
// breaks in it drawing at all.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::{Cell, OnceCell, RefCell};

use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::number::Number;
use super::visit;

/// Where a row of the table is: which record it shows, how far its top edge is
/// from the top of the list, and how tall it is. A list view puts the row it is
/// part way through at a negative offset, which is how far into it the view has
/// scrolled.
struct Placed {
    position: u32,
    row: usize,
    y: f32,
    height: f32,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Gutter {
        /// The table, which is a child of this widget rather than its neighbour,
        /// because a neighbour is laid out in whatever order the box it is in
        /// decides and this has to be after.
        pub table: RefCell<Option<gtk::Widget>>,
        /// The view inside the table, which is what actually knows where the
        /// rows are. Looked up once; the table's insides do not change.
        pub view: OnceCell<gtk::ColumnView>,
        /// The line between the numbers and the table. A widget rather than
        /// something drawn, so the theme decides what a divider looks like.
        pub divider: RefCell<Option<gtk::Separator>>,
        /// One number per row on screen, kept rather than rebuilt: the count
        /// changes by a row or two a frame and a pool costs nothing.
        pub numbers: RefCell<Vec<Number>>,
        /// How wide the numbers are, in digits. Sized to the widest number the
        /// file can show rather than to the ones on screen, so the gutter does
        /// not change width as you scroll.
        pub digits: Cell<i32>,
        /// Which row of the view the keyboard is on, which is the one lit up.
        pub current: Cell<Option<u32>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Gutter {
        const NAME: &'static str = "CommaGutter";
        type Type = super::Gutter;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Buildable,);
    }

    impl ObjectImpl for Gutter {
        fn constructed(&self) {
            self.parent_constructed();

            let divider = gtk::Separator::new(gtk::Orientation::Vertical);
            divider.set_parent(&*self.obj());
            self.divider.replace(Some(divider));

            self.obj().add_css_class("gutter");
            self.obj().connect_wheel();
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Gutter {
        /// As wide as the numbers and the table together, and as tall as the
        /// table. The numbers ask for no height of their own: they are given the
        /// height of the row they name.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let beside = self.obj().numbers_width();
            let Some(table) = self.table.borrow().clone() else {
                return (beside, beside, -1, -1);
            };

            if orientation == gtk::Orientation::Horizontal {
                let (least, wanted, _, _) = table.measure(orientation, for_size);
                return (least + beside, wanted + beside, -1, -1);
            }
            let left = if for_size < 0 {
                for_size
            } else {
                (for_size - beside).max(0)
            };
            table.measure(orientation, left)
        }

        /// The table first, then a number against each row it drew.
        ///
        /// This order is the point of the widget. Allocating a widget lays out
        /// everything inside it there and then, so by the time the table returns
        /// its rows are where this frame puts them, and the numbers can be put
        /// beside them rather than beside where they were last frame.
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let beside = self.obj().numbers_width();
            let Some(table) = self.table.borrow().clone() else {
                return;
            };

            table.allocate(
                (width - beside).max(0),
                height,
                baseline,
                Some(super::shift(beside as f32, 0.0)),
            );
            if let Some(divider) = self.divider.borrow().as_ref() {
                divider.allocate(
                    1,
                    height,
                    baseline,
                    Some(super::shift((beside - 1) as f32, 0.0)),
                );
            }
            self.obj().place_numbers(beside, baseline);
        }

        /// The numbers are clipped to the rows rather than to this widget: the
        /// row a list view is part way through hangs above the top of it, and
        /// without a clip its number would be drawn over the column headings.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            if let Some(table) = self.table.borrow().as_ref() {
                obj.snapshot_child(table, snapshot);
            }
            if let Some(divider) = self.divider.borrow().as_ref() {
                obj.snapshot_child(divider, snapshot);
            }

            if let Some(rows) = obj.rows_area() {
                snapshot.push_clip(&rows);
                for number in self.numbers.borrow().iter() {
                    if number.is_child_visible() {
                        obj.snapshot_child(number, snapshot);
                    }
                }
                snapshot.pop();
            }
        }
    }

    impl BuildableImpl for Gutter {
        /// The table is the one child the blueprint gives this, and it goes in
        /// the slot rather than on the end of a list.
        fn add_child(&self, builder: &gtk::Builder, child: &glib::Object, kind: Option<&str>) {
            let Some(widget) = child.downcast_ref::<gtk::Widget>() else {
                return self.parent_add_child(builder, child, kind);
            };
            widget.insert_before(&*self.obj(), self.divider.borrow().as_ref());
            self.table.replace(Some(widget.clone()));
        }
    }
}

glib::wrapper! {
    pub struct Gutter(ObjectSubclass<imp::Gutter>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Gutter {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Gutter {
    /// Says how wide the numbers have to be, for a file of however many rows it
    /// has now. A file crossing a thousand rows widens them where they are.
    pub fn set_digits(&self, digits: i32) {
        if self.imp().digits.replace(digits) == digits {
            return;
        }
        for number in self.imp().numbers.borrow().iter() {
            number.set_digits(digits);
        }
        self.queue_resize();
    }

    /// Says which row of the view the keyboard is on, so that the number beside
    /// it is lit along with the row itself.
    ///
    /// The table lights its own row from `:focus-within`, which it can do
    /// because the cell with the keyboard in it is inside that row. Nothing here
    /// is inside anything, so the numbers are told.
    pub fn set_current(&self, position: Option<u32>) {
        if self.imp().current.replace(position) == position {
            return;
        }
        self.queue_allocate();
    }

    /// How wide the numbers are, which is what one of them asks for. Measured
    /// from a real number rather than reckoned from the digit count, so it is
    /// the same answer the number will give when it is drawn.
    fn numbers_width(&self) -> i32 {
        let numbers = self.imp().numbers.borrow();
        let ruler = match numbers.first() {
            Some(number) => number.clone(),
            None => {
                drop(numbers);
                return self.grow_pool(1).map_or(0, |number| {
                    number.measure(gtk::Orientation::Horizontal, -1).1
                });
            }
        };
        ruler.measure(gtk::Orientation::Horizontal, -1).1
    }

    /// Puts a number against every row the table is drawing, and hides the ones
    /// left over.
    fn place_numbers(&self, width: i32, baseline: i32) {
        let Some(list) = self.list() else {
            return;
        };
        let Some(top) = list.compute_point(self, &graphene::Point::new(0.0, 0.0)) else {
            return;
        };

        let drawn = self.showing(&list);
        self.grow_pool(drawn.len());
        let current = self.imp().current.get();
        let numbers = self.imp().numbers.borrow();

        for (number, row) in numbers.iter().zip(drawn.iter()) {
            number.set_child_visible(true);
            number.show_row(row.position, row.row, current == Some(row.position));
            number.allocate(
                width,
                row.height as i32,
                baseline,
                Some(shift(0.0, top.y() + row.y)),
            );
        }
        for number in numbers.iter().skip(drawn.len()) {
            number.set_child_visible(false);
        }
    }

    /// Makes sure there are at least this many numbers to hand, and gives back
    /// the first of them.
    fn grow_pool(&self, wanted: usize) -> Option<Number> {
        let mut numbers = self.imp().numbers.borrow_mut();
        while numbers.len() < wanted {
            let number = Number::new();
            number.set_digits(self.imp().digits.get());
            number.set_parent(self);
            numbers.push(number);
        }
        numbers.first().cloned()
    }

    /// The rows the table is drawing, in the order they are drawn.
    ///
    /// The view's own list is asked for its children rather than the whole
    /// widget tree being walked, because a walk turns up rows GTK is holding to
    /// hand out again, which answer for wherever they were last used. Those are
    /// unmapped, and so is anything else not being drawn, which is the test.
    fn showing(&self, list: &gtk::Widget) -> Vec<Placed> {
        let mut rows = Vec::new();
        let mut child = list.first_child();
        while let Some(row) = child {
            child = row.next_sibling();
            if !row.is_mapped() {
                continue;
            }
            let placed = super::position_in(&row).zip(row.compute_bounds(list));
            if let Some(((position, index), bounds)) = placed {
                rows.push(Placed {
                    position,
                    row: index,
                    y: bounds.y(),
                    height: bounds.height(),
                });
            }
        }
        rows
    }

    /// Where the rows are, in this widget's own coordinates, which is what the
    /// numbers are clipped to.
    fn rows_area(&self) -> Option<graphene::Rect> {
        let bounds = self.list()?.compute_bounds(self)?;
        Some(graphene::Rect::new(
            0.0,
            bounds.y(),
            self.numbers_width() as f32,
            bounds.height(),
        ))
    }

    /// The part of the table's view that holds the rows, as against the row of
    /// column titles above them. Asked for by what it is rather than by where it
    /// sits, so a view that grows another child keeps working.
    fn list(&self) -> Option<gtk::Widget> {
        let view = self.view()?;
        let mut child = view.first_child();
        while let Some(current) = child {
            if current.css_name().as_str() == "listview" {
                return Some(current);
            }
            child = current.next_sibling();
        }
        None
    }

    /// The view inside the table.
    fn view(&self) -> Option<gtk::ColumnView> {
        let imp = self.imp();
        if let Some(view) = imp.view.get() {
            return Some(view.clone());
        }
        let table = imp.table.borrow().clone()?;
        let mut found = None;
        visit(&table, &mut |view: &gtk::ColumnView| {
            found = Some(view.clone());
            glib::ControlFlow::Break
        });
        let found = found?;
        let _ = imp.view.set(found.clone());
        Some(found)
    }

    /// A wheel over the numbers scrolls the table, the way it does from anywhere
    /// else in the grid. They are beside the table rather than inside it, so
    /// nothing else would carry the event there.
    fn connect_wheel(&self) {
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        wheel.connect_scroll(glib::clone!(
            #[weak(rename_to = gutter)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, down| {
                let Some(scroll) = gutter.view().and_then(|view| view.vadjustment()) else {
                    return glib::Propagation::Proceed;
                };
                // The table's own step, so a click of the wheel moves the same
                // distance whichever half of the grid it is over.
                let step = match scroll.step_increment() {
                    0.0 => scroll.page_size() / 10.0,
                    step => step,
                };
                scroll.set_value(step.mul_add(down, scroll.value()));
                glib::Propagation::Stop
            }
        ));
        self.add_controller(wheel);
    }
}

/// A transform that moves a child to where it goes, which is all the placing
/// here needs.
fn shift(x: f32, y: f32) -> gtk::gsk::Transform {
    gtk::gsk::Transform::new().translate(&graphene::Point::new(x, y))
}
