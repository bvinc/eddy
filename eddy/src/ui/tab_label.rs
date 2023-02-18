use crate::theme::Theme;
use eddy_workspace::style::{Attr, AttrSpan};
use eddy_workspace::{BufferId, BufferUpdate, Event, ViewId, Workspace};
use gdk::{Key, ModifierType};
use glib::Object;
use glib::{clone, Sender};
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, Adjustment};
use log::*;
use lru_cache::LruCache;
use once_cell::unsync::OnceCell;
use pango::Attribute;
use ropey::RopeSlice;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::cmp::{max, min};
use std::rc::Rc;
use std::time::Instant;

glib::wrapper! {
    pub struct TabLabel(ObjectSubclass<imp::TabLabel>)
    @extends gtk::Box, gtk::Widget;
}

impl TabLabel {
    pub fn new(workspace: Rc<RefCell<Workspace>>, view_id: ViewId) -> Self {
        let obj = glib::Object::new::<Self>();
        let imp = imp::TabLabel::from_obj(&obj);
        imp.workspace.set(workspace.clone());
        imp.view_id.set(view_id);

        obj.setup_widgets();
        obj
    }

    fn bu_ls_initialized(&self) {
        debug!("ls initialized");
    }

    fn bu_path_changed(&self, buf_id: BufferId) {
        debug!("path changed");
    }

    fn bu_pristine_changed(&self, buf_id: BufferId) {
        debug!("pristine changed");
        self.emit_by_name::<()>("pristine-changed", &[]);
    }

    pub fn view_id(&self) -> usize {
        let code_view_priv = imp::TabLabel::from_obj(&self);
        code_view_priv.view_id.get()
    }

    fn setup_widgets(&self) {
        self.set_spacing(4);
        let view_id = self.imp().view_id.get();
        let workspace = self.imp().workspace.get().unwrap();
        let name = workspace.borrow().display_name(view_id);
        self.append(&gtk::Label::new(Some(&name)));
        let button = gtk::Button::new();
        self.append(&button);
        button.connect_clicked(clone!(@weak workspace as workspace => move |_| {
            workspace.borrow().close_view(view_id);
        }));
    }

    pub fn process_event(&self, event: &Event) {
        let buffer_id = self
            .imp()
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.imp().view_id.get())
            .borrow()
            .id;
        match event {
            _ => {}
        }
    }
}

mod imp {
    use crate::theme::Theme;
    use eddy_workspace::style::{Attr, AttrSpan};
    use eddy_workspace::{ViewId, Workspace};
    use gdk::{Key, ModifierType};
    use glib::{clone, Sender};
    use gtk::glib::subclass;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, glib, Adjustment};
    use log::*;
    use lru_cache::LruCache;
    use once_cell::unsync::OnceCell;
    use pango::Attribute;
    use ropey::RopeSlice;
    use std::borrow::Cow;
    use std::cell::{Cell, RefCell};
    use std::cmp::{max, min};
    use std::rc::Rc;
    use std::time::Instant;

    pub struct TabLabel {
        pub workspace: OnceCell<Rc<RefCell<Workspace>>>,
        pub view_id: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TabLabel {
        const NAME: &'static str = "TabLabel";
        type Type = super::TabLabel;
        type ParentType = gtk::Box;
        type Instance = subclass::basic::InstanceStruct<Self>;
        type Class = subclass::basic::ClassStruct<Self>;

        fn new() -> Self {
            let workspace = OnceCell::new();
            let view_id = Cell::new(0);

            Self { workspace, view_id }
        }
    }

    impl ObjectImpl for TabLabel {}
    impl WidgetImpl for TabLabel {}
    impl BoxImpl for TabLabel {}
    impl TabLabel {}
}
