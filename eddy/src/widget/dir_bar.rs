use crate::Win;
use log::*;
use anyhow::bail;
use glib::clone;
use gtk::prelude::*;
use gtk::TreeModelSort;
use gtk::{
    Adjustment, CellRendererText, ScrolledWindow, SortColumn, SortType, TreeIter, 
    TreePath, TreeStore, TreeView, TreeViewColumn,
};
use relm::{connect, Relm, Update, Widget};
use relm_derive::Msg;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct DirBar {
    model: Model,
    scrolled_window: ScrolledWindow,
    tree_view: TreeView,
}

pub struct Model {
    dir: Rc<RefCell<Option<PathBuf>>>,
    parent_relm: Relm<crate::Win>,
    tree_store: TreeStore,
}

#[derive(Msg)]
pub enum Msg {
    SetDir(PathBuf),
    RowActivated(TreePath, TreeViewColumn),
}

impl Update for DirBar {
    type Model = Model;
    type ModelParam = Relm<Win>;
    type Msg = Msg;

    fn model(_: &Relm<Self>, param: Relm<Win>) -> Model {
        let tree_store = TreeStore::new(&[String::static_type(), bool::static_type()]);
        let tree_model_sort = TreeModelSort::new(&tree_store);
        tree_model_sort.set_sort_column_id(SortColumn::Index(0), SortType::Ascending);

        // Compare based off of the string value.  This is required because the
        // default GTK sort seems to ignore leading dots.
        tree_model_sort.set_sort_func(SortColumn::Index(0), |m, i1, i2| {
            let v1: Option<String> = m.get_value(i1, 0).get().unwrap();
            let v2: Option<String> = m.get_value(i2, 0).get().unwrap();
            return v1.cmp(&v2);
        });
        Model {
            dir: Rc::new(RefCell::new(None)),
            parent_relm: param,
            tree_store: tree_store.clone(),
        }
    }

    fn update(&mut self, event: Msg) {
        if let Err(e) = match event {
            Msg::SetDir(ref p) => self.set_dir(Some(&p)),
            Msg::RowActivated(tp, tvc) => self.row_activated(tp, tvc),
        } {
            warn!("{}", e);
        }
    }
}

impl Widget for DirBar {
    // Specify the type of the root widget.
    type Root = ScrolledWindow;

    // Return the root widget.
    fn root(&self) -> Self::Root {
        self.scrolled_window.clone()
    }

    // Create the widgets.
    fn view(relm: &Relm<Self>, model: Self::Model) -> Self {
        let scrolled_window = ScrolledWindow::new(None::<&Adjustment>, None::<&Adjustment>);

        let column0 = TreeViewColumn::new();
        let cell0 = CellRendererText::new();
        column0.pack_start(&cell0, true);
        column0.add_attribute(&cell0, "text", 0);

        let tree_view = TreeView::new();
        tree_view.set_model(Some(&model.tree_store));
        tree_view.set_headers_visible(false);

        tree_view.append_column(&column0);

        let dir = model.dir.clone();
        let tree_store = model.tree_store.clone();
        tree_view.connect_test_expand_row(clone!(@strong dir => move |tv, ti, tp| {
            // let mut state = state.borrow_mut();
            handle_test_expand_row(&tv, dir.borrow().clone(), &tree_store, &ti, &tp)
        }));

        // connect!(
        //     relm,
        //     tree_view,
        //     connect_test_expand_row(_, ti, tp),
        //     return (Msg::TestExpandRow(ti.clone(), tp.clone()), Inhibit(true))
        // );

        // let dir = model.dir.clone();
        // let tree_store = model.tree_store.clone();
        // tree_view.connect_row_expanded(clone!(@strong dir => move |tv, ti, tp| {
        //     // let mut state = state.borrow_mut();
        //     handle_row_expanded(&tv, dir.borrow().clone(), &tree_store, &ti, &tp);
        // }));

        // connect!(
        //     relm,
        //     tree_view,
        //     connect_row_expanded(_, ti, tp),
        //     // Msg::RowExpanded(ti.clone(), tp.clone())
        //     return row_expanded2(ti.clone, tp.clone())
        // );
        connect!(
            relm,
            tree_view,
            connect_row_activated(_, tp, tvc),
            Msg::RowActivated(tp.clone(), tvc.clone())
        );

        scrolled_window.add(&tree_view);

        scrolled_window.show_all();

        DirBar {
            model,
            scrolled_window,
            tree_view,
        }
    }
}

impl DirBar {
    pub fn set_dir(&mut self, dir: Option<&Path>) -> Result<(), anyhow::Error> {
        *self.model.dir.borrow_mut() = dir.map(|p| p.canonicalize()).transpose()?;
        println!("set dir {:?}", self.model.dir);
        //self.refresh()?;
        if let Some(dir) = dir {
            refresh_dir(&self.model.tree_store, None, dir)?;
        }
        Ok(())
    }

    pub fn row_activated(&mut self, tp: TreePath, _: TreeViewColumn) -> Result<(), anyhow::Error> {
        let dir = self.model.dir.borrow().clone();
            if let Some(ref ti) = self.model.tree_store.get_iter(&tp) {
                if self.model.tree_store.iter_has_child(&ti) {
                    if !self.tree_view.row_expanded(&tp) {
                        self.tree_view.expand_row(&tp, false);
                    } else {
                        self.tree_view.collapse_row(&tp);
                    }
                    return Ok(());
                }
            } else {
                bail!("invalid path")
            }
            let path = tree_path_to_path(dir.as_ref(), &self.model.tree_store, &tp)?;
            self.model
                .parent_relm
                .stream()
                .emit(crate::Msg::OpenPath(path));
            Ok(())
        }
}

fn handle_test_expand_row(
    _: &TreeView,
    dir: Option<PathBuf>,
    tree_store: &TreeStore,
    ti: &TreeIter,
    tp: &TreePath,
) -> Inhibit {
    dbg!("handle_test_expand_row");
    if let Ok(path) = tree_path_to_path(dir, &tree_store, &tp) {
        if let Err(e) = refresh_dir(&tree_store, Some(ti), &path) {
            warn!("{}", e);
        }
    }
    Inhibit(false)
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
        let node = tree_store.insert_with_values(
            ti,
            None,
            &[0],
            &[&fname.to_string_lossy().to_string()],
        );
        if is_dir {
            tree_store.insert_with_values(Some(&node), None, &[0], &[&"."]);
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
        let mut ti = tree_store.get_iter(tp).unwrap();
        loop {
            let val = tree_store.get_value(&ti, 0);
            let s: String = val.get()?.unwrap();
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
