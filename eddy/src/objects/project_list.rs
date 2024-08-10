use gflux::ComponentCtx;
use gtk::subclass::prelude::*;

use crate::components::dirbar2::DirBarComponent;

mod imp {
    use gflux::ComponentCtx;
    use gio::subclass::prelude::*;
    use glib::subclass::prelude::*;
    use glib::{Properties, StaticType};
    use gtk::subclass::prelude::*;
    use gtk::{prelude::*, StringObject};
    use once_cell::unsync::OnceCell;
    use std::cell::Cell;

    use crate::components::dirbar2::DirBarComponent;

    #[derive(Default)]
    pub struct ProjectGList {
        pub ctx: OnceCell<ComponentCtx<DirBarComponent>>,
    }

    // Object subclass implementation
    #[glib::object_subclass]
    impl ObjectSubclass for ProjectGList {
        const ABSTRACT: bool = false;
        const NAME: &'static str = "ProjectGList";
        type Type = super::ProjectGList;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ProjectGList {}
    impl ListModelImpl for ProjectGList {
        fn item_type(&self) -> glib::Type {
            StringObject::static_type()
        }

        fn n_items(&self) -> u32 {
            self.ctx.get().unwrap().with_model(|m| m.projects.len()) as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let idx: usize = position as usize;
            let name = self
                .ctx
                .get()
                .unwrap()
                .with_model(|m| m.projects.get(&idx).map(|p| p.name.clone()))?;
            let gname = glib::GString::from_string_checked(name).ok()?;
            let soname = StringObject::from(gname);
            Some(soname.upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct ProjectGList(ObjectSubclass<imp::ProjectGList>)
        @implements gio::ListModel;
}

impl ProjectGList {
    pub fn new(ctx: ComponentCtx<DirBarComponent>) -> Self {
        let instance = glib::Object::new::<ProjectGList>();
        instance.imp().ctx.set(ctx).unwrap();
        instance
    }
}
