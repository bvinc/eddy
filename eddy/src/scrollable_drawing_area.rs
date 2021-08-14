//! This file creates a `ScrollableDrawingArea` which subclasses from
//! `DrawingArea` and implements the `Scrollable` interface.

use gio::prelude::*;
use glib::glib_wrapper;
use glib::subclass;
use glib::subclass::object::Property;
use glib::subclass::prelude::*;
use glib::translate::*;
use glib::{
    glib_object_impl, glib_object_subclass, glib_object_wrapper, Object, ParamFlags, ParamSpec,
    Value,
};
use gtk::subclass::prelude::*;
use gtk::{Adjustment, ScrollablePolicy};
use log::*;
use once_cell::unsync::OnceCell;
use std::cell::{Cell, RefCell};
use std::ptr;

static PROPERTIES: [Property; 4] = [
    Property("hadjustment", |name| {
        ParamSpec::object(
            name,
            name,
            name,
            Adjustment::static_type(),
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT,
        )
    }),
    Property("hscroll-policy", |name| {
        ParamSpec::enum_(
            name,
            name,
            name,
            ScrollablePolicy::static_type(),
            ScrollablePolicy::Minimum.to_glib(),
            ParamFlags::READWRITE,
        )
    }),
    Property("vadjustment", |name| {
        ParamSpec::object(
            name,
            name,
            name,
            Adjustment::static_type(),
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT,
        )
    }),
    Property("vscroll-policy", |name| {
        ParamSpec::enum_(
            name,
            name,
            name,
            ScrollablePolicy::static_type(),
            ScrollablePolicy::Minimum.to_glib(),
            ParamFlags::READWRITE,
        )
    }),
];

#[derive(Debug)]
struct Widgets {
    hadjustment: RefCell<Adjustment>,
    hscroll_policy: Cell<ScrollablePolicy>,
    vadjustment: RefCell<Adjustment>,
    vscroll_policy: Cell<ScrollablePolicy>,
}

// This is the private part of our `ScrollableDrawingArea` object.
// Its where state and widgets are stored when they don't
// need to be publicly accesible.
#[derive(Debug)]
pub struct ScrollableDrawingAreaPrivate {
    widgets: OnceCell<Widgets>,
    counter: Cell<u64>,
}

impl ObjectSubclass for ScrollableDrawingAreaPrivate {
    const NAME: &'static str = "ScrollableDrawingAreaPrivate";
    type ParentType = gtk::DrawingArea;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    glib_object_subclass!();

    fn type_init(type_: &mut subclass::InitializingType<Self>) {
        // This is what I would like to call, but I can't since Scrollable is
        // not IsImplementable.
        // type_.add_interface::<gtk::Scrollable>();
        // The following will have to suffice.

        let interface_info = gobject_sys::GInterfaceInfo {
            interface_init: None,            //TODO
            interface_finalize: None,        //TODO
            interface_data: ptr::null_mut(), //TODO
        };

        unsafe {
            gobject_sys::g_type_add_interface_static(
                type_.to_glib(),
                <gtk::Scrollable as glib::StaticType>::static_type().to_glib(),
                &interface_info,
            );
        }
    }

    fn class_init(class: &mut Self::Class) {
        class.install_properties(&PROPERTIES);
    }

    fn new() -> Self {
        Self {
            widgets: OnceCell::new(),
            counter: Cell::new(0),
        }
    }
}

impl ObjectImpl for ScrollableDrawingAreaPrivate {
    glib_object_impl!();

    // Here we are overriding the glib::Object::contructed
    // method. Its what gets called when we create our Object
    // and where we can initialize things.
    fn constructed(&self, obj: &glib::Object) {
        trace!("ScrollableDrawingAreaPrivate::constructed");
        self.parent_constructed(obj);

        let hadjustment = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let hscroll_policy = Cell::new(ScrollablePolicy::Minimum);
        let vadjustment = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let vscroll_policy = Cell::new(ScrollablePolicy::Minimum);

        self.widgets
            .set(Widgets {
                hadjustment,
                hscroll_policy,
                vadjustment,
                vscroll_policy,
            })
            .expect("failed to initialize window state");
    }

    fn set_property(&self, _obj: &Object, id: usize, value: &Value) {
        trace!("ScrollableDrawingAreaPrivate::set_property");
        let prop = &PROPERTIES[id];
        match *prop {
            Property("hadjustment", ..) => {
                if let Ok(Some(value)) = value.get::<Adjustment>() {
                    *self.widgets.get().unwrap().hadjustment.borrow_mut() = value.clone();
                }
            }
            Property("hscroll-policy", ..) => {
                if let Ok(Some(value)) = value.get::<ScrollablePolicy>() {
                    self.widgets.get().unwrap().hscroll_policy.set(value);
                }
            }
            Property("vadjustment", ..) => {
                if let Ok(Some(value)) = value.get::<Adjustment>() {
                    *self.widgets.get().unwrap().vadjustment.borrow_mut() = value.clone();
                }
            }
            Property("vscroll-policy", ..) => {
                if let Ok(Some(value)) = value.get::<ScrollablePolicy>() {
                    self.widgets.get().unwrap().vscroll_policy.set(value);
                }
            }
            _ => {}
        }
    }

    fn get_property(&self, _obj: &Object, id: usize) -> Result<Value, ()> {
        trace!("ScrollableDrawingAreaPrivate::get_property(id={})", id);
        trace!(
            "ScrollableDrawingAreaPrivate::get_property hadj={:?}, vadj={:?}",
            self.widgets.get().unwrap().hadjustment.borrow().to_value(),
            self.widgets.get().unwrap().vadjustment.borrow().to_value()
        );
        let prop = &PROPERTIES[id];
        match *prop {
            Property("hadjustment", ..) => {
                Ok(self.widgets.get().unwrap().hadjustment.borrow().to_value())
            }
            Property("hscroll-policy", ..) => {
                Ok(self.widgets.get().unwrap().hscroll_policy.get().to_value())
            }
            Property("vadjustment", ..) => {
                Ok(self.widgets.get().unwrap().vadjustment.borrow().to_value())
            }
            Property("vscroll-policy", ..) => {
                Ok(self.widgets.get().unwrap().vscroll_policy.get().to_value())
            }
            _ => Err(()),
        }
    }
}

impl ScrollableDrawingAreaPrivate {}

impl WidgetImpl for ScrollableDrawingAreaPrivate {}
impl DrawingAreaImpl for ScrollableDrawingAreaPrivate {}

glib_wrapper! {
    pub struct ScrollableDrawingArea(
        Object<subclass::basic::InstanceStruct<ScrollableDrawingAreaPrivate>,
        subclass::basic::ClassStruct<ScrollableDrawingAreaPrivate>,
        ScrollableDrawingAreaClass>)
        @extends gtk::Widget, gtk::DrawingArea,
        @implements gtk::Scrollable;

    match fn {
        get_type => || ScrollableDrawingAreaPrivate::get_type().to_glib(),
    }
}

impl ScrollableDrawingArea {
    pub fn new() -> Self {
        trace!("ScrollableDrawingArea::new");
        glib::Object::new(Self::static_type(), &[])
            .expect("failed to create ScrollableDrawingArea")
            .downcast::<ScrollableDrawingArea>()
            .expect("created ScrollableDrawingArea is of wrong type")
    }
}
