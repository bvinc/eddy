use gflux::ComponentCtx;

use crate::components::dirbar2::DirBarComponent;
use gtk::subclass::prelude::*;

mod imp {
    use gflux::ComponentCtx;
    use gio::subclass::prelude::*;
    use glib::subclass::prelude::*;
    use glib::Properties;
    use glib::types::StaticType;
    use gtk::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::StringObject;
    use once_cell::unsync::OnceCell;
    use std::cell::Cell;

    use crate::components::dirbar2::DirBarComponent;
    use crate::objects::file_node::FileNode;

    #[derive(Default)]
    pub struct FileNodeList {
        pub ctx: OnceCell<ComponentCtx<DirBarComponent>>,
    }

    // Object subclass implementation
    #[glib::object_subclass]
    impl ObjectSubclass for FileNodeList {
        const ABSTRACT: bool = false;
        const NAME: &'static str = "FileNodeList";
        type Type = super::FileNodeList;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for FileNodeList {}
    impl ListModelImpl for FileNodeList {
        fn item_type(&self) -> glib::Type {
            FileNode::static_type()
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
            let soname = FileNode::from(gname);
            Some(soname.upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct FileNodeList(ObjectSubclass<imp::FileNodeList>)
        @implements gio::ListModel;
}

impl FileNodeList {
    fn new(ctx: ComponentCtx<DirBarComponent>) -> Self {
        let instance = glib::Object::new::<FileNodeList>();
        instance.imp().ctx.set(ctx).unwrap();
        instance
    }
}
