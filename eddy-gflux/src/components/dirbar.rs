use super::window::WindowComponent;
use anyhow::bail;
use eddy_workspace::Workspace;
use gflux::{Component, ComponentCtx, ComponentHandle};
use glib::clone;
use gtk::{prelude::*, Inhibit};
use log::*;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[allow(dead_code)]
pub struct DirBarComponent {
    tree_view: gtk::TreeView,
    tree_store: gtk::TreeStore,
}

impl Component for DirBarComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = gtk::TreeView;
    type Params = ();

    fn widget(&self) -> Self::Widget {
        self.tree_view.clone()
    }

    fn build(ctx: ComponentCtx<Self>, _params: ()) -> Self {
        let tree_store = gtk::TreeStore::new(&[String::static_type(), bool::static_type()]);
        let tree_view = gtk::TreeView::new();
        let column0 = gtk::TreeViewColumn::new();
        let cell0 = gtk::CellRendererText::new();
        column0.pack_start(&cell0, true);
        column0.add_attribute(&cell0, "text", 0);

        // let tree_view = TreeView::new();
        tree_view.set_model(Some(&tree_store));
        tree_view.set_headers_visible(false);

        tree_view.append_column(&column0);

        tree_view.connect_test_expand_row(
            clone!(@strong ctx, @strong tree_store => move |tv, ti, tp| {
                dbg!("handle_test_expand_row");
                let dir = ctx.with_model(|ws| ws.dir.clone());

                if let Ok(path) = tree_path_to_path(Some(&dir), &tree_store, &tp) {
                    if let Err(e) = refresh_dir(&tree_store, Some(ti), &path) {
                        warn!("{}", e);
                    }
                }
                Inhibit(false)
            }),
        );

        tree_view.connect_test_collapse_row(|tv, ti, tp| Inhibit(false));

        tree_view.connect_row_activated(
            clone!(@strong ctx, @strong tree_store => move |tv, tp, tvc| {
                let dir = ctx.with_model(|ws| ws.dir.clone());
                if let Some(ref ti) = tree_store.iter(&tp) {
                    if tree_store.iter_has_child(&ti) {
                        if !tv.row_expanded(&tp) {
                            tv.expand_row(&tp, false);
                        } else {
                            tv.collapse_row(&tp);
                        }
                        return;
                    }
                } else {
                    dbg!("invalid path");
                    return;
                }
                match tree_path_to_path(Some(&dir), &tree_store, &tp) {
                    Ok(path) => {
                        if let Ok(path) = path.canonicalize() {
                            dbg!(&path);
                            ctx.with_model_mut(|ws| ws.new_view(Some(&path)));
                        }
                    }
                    Err(e) => {
                        error!("tree to path: {}", e);
                    }
                };
            }),
        );

        let dir = ctx.with_model(|ws| ws.dir.clone());

        // TODO be able to show an error if one happens
        let _ = refresh_dir(&tree_store, None, &dir);

        Self {
            tree_view,
            tree_store,
        }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {}
}

/// Given a path in the tree, clear it of its children, and re-read the
/// files from the disk.
pub fn refresh_dir(
    tree_store: &gtk::TreeStore,
    ti: Option<&gtk::TreeIter>,
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
    tree_store: &gtk::TreeStore,
    tp: &gtk::TreePath,
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
pub fn clear_tree_iter_children(tree_store: &gtk::TreeStore, ti: Option<&gtk::TreeIter>) {
    let mut pi = tree_store.iter_children(ti);
    if let Some(ref mut p) = pi {
        dbg!("starting remove");
        while tree_store.remove(p) {}

        return;
    }
}
