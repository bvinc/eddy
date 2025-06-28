use std::path::PathBuf;

use eddy_model::ProjectId;

use gtk::subclass::prelude::*;

mod imp {
    use eddy_model::ProjectId;
    use gio::subclass::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;

    #[derive(Default)]
    pub struct FileNode {
        pub proj_name: RefCell<String>,
        pub proj_id: Cell<ProjectId>,
        pub path: RefCell<PathBuf>,
        pub is_dir: Cell<bool>,
    }

    // Object subclass implementation
    #[glib::object_subclass]
    impl ObjectSubclass for FileNode {
        const ABSTRACT: bool = false;
        const NAME: &'static str = "FileNode";
        type Type = super::FileNode;
        type ParentType = glib::Object;
        type Interfaces = ();
    }

    impl ObjectImpl for FileNode {}
}

glib::wrapper! {
    pub struct FileNode(ObjectSubclass<imp::FileNode>);
}

impl FileNode {
    pub fn new(proj_name: &str, proj_id: ProjectId, path: PathBuf, is_dir: bool) -> Self {
        let instance = glib::Object::new::<FileNode>();
        instance.imp().proj_name.replace(proj_name.to_string());
        instance.imp().proj_id.set(proj_id);
        instance.imp().path.replace(path);
        instance.imp().is_dir.set(is_dir);
        instance
    }

    pub fn proj_name(&self) -> String {
        self.imp().proj_name.borrow().clone()
    }
    pub fn proj_id(&self) -> ProjectId {
        self.imp().proj_id.get()
    }
    pub fn path(&self) -> PathBuf {
        self.imp().path.borrow().clone()
    }
    pub fn is_dir(&self) -> bool {
        self.imp().is_dir.get()
    }
}
