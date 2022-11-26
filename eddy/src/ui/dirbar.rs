use anyhow::bail;
use glib::Sender;
use gtk::glib;
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{CellRendererText, TreeIter, TreePath, TreeStore, TreeViewColumn};
use gtk_macros::send;
use log::*;
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::app::{EddyApplication, Event};

pub struct DirBarPrivate {
    sender: OnceCell<Sender<Event>>,
    dir: Rc<RefCell<Option<PathBuf>>>,
    tree_store: TreeStore,
}

#[glib::object_subclass]
impl ObjectSubclass for DirBarPrivate {
    const NAME: &'static str = "DirBar";
    type Type = DirBar;
    type ParentType = gtk::TreeView;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        let mut pb = PathBuf::new();
        pb.push(".");

        let tree_store = TreeStore::new(&[String::static_type(), bool::static_type()]);
        Self {
            sender: OnceCell::new(),
            dir: Rc::new(RefCell::new(Some(pb))),
            tree_store: tree_store.clone(),
        }
    }
}

impl ObjectImpl for DirBarPrivate {}
impl WidgetImpl for DirBarPrivate {}
impl WindowImpl for DirBarPrivate {}
impl gtk::subclass::prelude::TreeViewImpl for DirBarPrivate {
    fn test_expand_row(&self, ti: &TreeIter, tp: &TreePath) -> bool {
        dbg!("handle_test_expand_row");
        if let Ok(path) = tree_path_to_path(self.dir.borrow().clone(), &self.tree_store, &tp) {
            if let Err(e) = refresh_dir(&self.tree_store, Some(ti), &path) {
                warn!("{}", e);
            }
        }
        false
    }

    fn test_collapse_row(&self, _ti: &TreeIter, _tp: &TreePath) -> bool {
        false
    }

    fn row_activated(&self, tp: &TreePath, _: &TreeViewColumn) {
        let dir = self.dir.borrow().clone();
        if let Some(ref ti) = self.tree_store.iter(&tp) {
            if self.tree_store.iter_has_child(&ti) {
                if !self.obj().row_expanded(&tp) {
                    self.obj().expand_row(&tp, false);
                } else {
                    self.obj().collapse_row(&tp);
                }
                return;
            }
        } else {
            dbg!("invalid path");
            return;
        }
        match tree_path_to_path(dir.as_ref(), &self.tree_store, &tp) {
            Ok(path) => {
                // send parent an OpenPath signal
                if let Ok(path) = path.canonicalize() {
                    dbg!(&path);
                    send!(self.sender.get().unwrap(), Event::Open(path));
                }
            }
            Err(e) => {
                error!("tree to path: {}", e);
            }
        };
    }
}

glib::wrapper! {
    pub struct DirBar(ObjectSubclass<DirBarPrivate>)
    @extends gtk::Widget, gtk::TreeView;
}

impl DirBar {
    pub fn new() -> Self {
        let dir_bar = glib::Object::new::<Self>(&[]);

        dir_bar.setup_widgets();
        dir_bar.setup_signals();
        dir_bar
    }

    pub fn init(&self, sender: Sender<Event>) {
        let self_ = DirBarPrivate::from_instance(self);

        let _ = self_.sender.set(sender);
    }

    fn setup_widgets(&self) {
        let self_ = DirBarPrivate::from_instance(self);

        let column0 = TreeViewColumn::new();
        let cell0 = CellRendererText::new();
        column0.pack_start(&cell0, true);
        column0.add_attribute(&cell0, "text", 0);

        // let tree_view = TreeView::new();
        self.set_model(Some(&self_.tree_store));
        self.set_headers_visible(false);

        self.append_column(&column0);

        let dir = self_.dir.clone();
        let tree_store = self_.tree_store.clone();

        // TODO be able to show an error if one happens
        let _ = refresh_dir(&tree_store, None, dir.borrow().as_ref().unwrap());
    }
    fn setup_signals(&self) {}
}

impl DirBar {
    pub fn set_dir(&mut self, dir: Option<&Path>) -> Result<(), anyhow::Error> {
        let self_ = DirBarPrivate::from_instance(self);
        *self_.dir.borrow_mut() = dir.map(|p| p.canonicalize()).transpose()?;
        println!("set dir {:?}", self_.dir);
        //self.refresh()?;
        if let Some(dir) = dir {
            refresh_dir(&self_.tree_store, None, dir)?;
        }
        Ok(())
    }
}

/// Given a path in the tree, clear it of its children, and re-read the
/// files from the disk.
pub fn refresh_dir(
    tree_store: &TreeStore,
    ti: Option<&TreeIter>,
    path: &Path,
) -> Result<(), anyhow::Error> {
    dbg!("refresh_dir");
    dbg!("clearing children");
    clear_tree_iter_children(&tree_store, ti);

    let mut files = vec![];
    dbg!(&path);
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        dbg!(&entry);
        let metadata = entry.metadata()?;
        files.push((metadata.is_dir(), entry.file_name()));
    }

    files.sort_unstable_by_key(|(is_dir, fname)| (!is_dir, fname.to_string_lossy().to_uppercase()));
    for (is_dir, fname) in files {
        let node =
            tree_store.insert_with_values(ti, None, &[(0, &fname.to_string_lossy().to_string())]);
        if is_dir {
            tree_store.insert_with_values(Some(&node), None, &[(0, &".")]);
        }
    }
    Ok(())
}

/// Given a TreePath, convert it to a PathBuf.  This is probably broken on
/// non-utf8 file paths.
pub fn tree_path_to_path<P: AsRef<Path>>(
    dir: Option<P>,
    tree_store: &TreeStore,
    tp: &TreePath,
) -> Result<PathBuf, anyhow::Error> {
    if let Some(ref dir) = dir {
        let mut stack = vec![];
        let mut ti = tree_store.iter(tp).unwrap();
        loop {
            let s: String = tree_store.get(&ti, 0);
            stack.push(s);

            if let Some(parent_ti) = tree_store.iter_parent(&ti) {
                ti = parent_ti;
            } else {
                break;
            }
        }
        let mut pb = PathBuf::from(dir.as_ref());
        for s in stack.iter().rev() {
            pb.push(s);
        }
        Ok(pb)
    } else {
        bail!("no directory opened")
    }
}

/// Clear a node in the tree of all of its children
pub fn clear_tree_iter_children(tree_store: &TreeStore, ti: Option<&TreeIter>) {
    let mut pi = tree_store.iter_children(ti);
    if let Some(ref mut p) = pi {
        dbg!("starting remove");
        while tree_store.remove(p) {}

        return;
    }
}
