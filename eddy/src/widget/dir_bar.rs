use crate::Win;
use anyhow::bail;
use gtk::prelude::*;
use gtk::TreeModelSort;
use gtk::{
    Adjustment, CellRendererText, ScrolledWindow, SortColumn, SortType, TreeIter, TreeModelFilter,
    TreePath, TreeStore, TreeView, TreeViewColumn,
};
use relm::{connect, Relm, Update, Widget};
use relm_derive::Msg;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DirBar {
    model: Model,
    scrolled_window: ScrolledWindow,
    tree_view: TreeView,
}

pub struct Model {
    dir: Option<PathBuf>,
    parent_relm: Relm<crate::Win>,
    tree_store: TreeStore,
    filter: TreeModelFilter,
}

#[derive(Msg)]
pub enum Msg {
    SetDir(PathBuf),
    RowExpanded(TreeIter, TreePath),
    RowActivated(TreePath, TreeViewColumn),
    TestExpandRow(TreeIter, TreePath),
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
        let filter = TreeModelFilter::new(&tree_store, None);
        // filter.set_visible_column(1);
        Model {
            dir: None,
            parent_relm: param,
            tree_store: tree_store.clone(),
            filter,
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::SetDir(ref p) => self.set_dir(Some(&p)),
            Msg::RowExpanded(ti, tp) => self.row_expanded(ti, tp),
            Msg::RowActivated(tp, tvc) => self.row_activated(tp, tvc),
            Msg::TestExpandRow(ti, tp) => self.test_expand_row(ti, tp),
        };
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
        tree_view.set_model(Some(&model.filter));
        tree_view.set_headers_visible(false);

        tree_view.append_column(&column0);

        // connect!(
        //     relm,
        //     tree_view,
        //     connect_test_expand_row(_, ti, tp),
        //     return (Msg::TestExpandRow(ti.clone(), tp.clone()), Inhibit(true))
        // );
        connect!(
            relm,
            tree_view,
            connect_row_expanded(_, ti, tp),
            Msg::RowExpanded(ti.clone(), tp.clone())
        );
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
        self.model.dir = dir.map(|p| p.canonicalize()).transpose()?;
        println!("set dir {:?}", self.model.dir);
        // self.model.tree_store = self.create_and_fill_model();
        // self.gtktreeview1.set_model(Some(&self.model.tree_store));
        // self.gtktreeview1.show_all();
        self.refresh()?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<(), anyhow::Error> {
        if let Some(ref dir) = self.model.dir {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let node = self.model.tree_store.insert_with_values(
                    None,
                    None,
                    &[0, 1],
                    &[&entry.file_name().to_string_lossy().to_string(), &true],
                );
                let metadata = entry.metadata()?;
                if metadata.is_dir() {
                    self.model.tree_store.insert_with_values(
                        Some(&node),
                        None,
                        &[0, 1],
                        &[&".", &false],
                    );
                }
            }
        }
        Ok(())
    }

    /// Clear a node in the tree of all of its children
    pub fn clear_tree_iter_children(&mut self, ti: &TreeIter) {
        let mut pi = self.model.tree_store.iter_children(Some(ti));
        if let Some(ref mut p) = pi {
            if !self.model.tree_store.remove(p) {
                return;
            }

            while self.model.tree_store.iter_next(p) {
                if !self.model.tree_store.remove(p) {
                    return;
                }
            }
        }
    }

    /// Given a path in the tree, clear it of its children, and re-read the
    /// files from the disk.
    pub fn refresh_dir(&mut self, tp: &TreePath, path: &Path) -> Result<(), anyhow::Error> {
        dbg!("refresh_dir");
        if let Some(ref ti) = self.model.tree_store.get_iter(tp) {
            // self.clear_tree_iter_children(ti);
            dbg!("adding dot");
            self.model
                .tree_store
                .insert_with_values(Some(ti), None, &[0, 1], &[&".", &false]);
            dbg!("added dot");
        }

        if let Some(ref ti) = self.model.tree_store.get_iter(tp) {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let node = self.model.tree_store.insert_with_values(
                    Some(ti),
                    None,
                    &[0, 1],
                    &[&entry.file_name().to_string_lossy().to_string(), &true],
                );
                let metadata = entry.metadata()?;
                if metadata.is_dir() {
                    self.model.tree_store.insert_with_values(
                        Some(&node),
                        None,
                        &[0, 1],
                        &[&".", &false],
                    );
                }
            }
            Ok(())
        } else {
            bail!("invalid tree path")
        }
    }

    /// Called when a row of the tree is expanded.
    pub fn test_expand_row(&mut self, _ti: TreeIter, tp: TreePath) -> Result<(), anyhow::Error> {
        dbg!("test_row_expanded");
        let path = self.tree_path_to_path(&tp)?;
        self.refresh_dir(&tp, &path)?;

        Ok(())
    }

    /// Called when a row of the tree is expanded.
    pub fn row_expanded(&mut self, _ti: TreeIter, tp: TreePath) -> Result<(), anyhow::Error> {
        dbg!("row_expanded");
        let path = self.tree_path_to_path(&tp)?;
        self.refresh_dir(&tp, &path)?;

        // // Why do I have to call expand_row here?  I don't know.  If I don't,
        // // then the process of removing the "." entry and adding the real
        // // entries causes the row expansion to not happen.  Adding this fixes it.
        // self.tree_view.expand_row(&tp, false);

        Ok(())
    }

    /// Given a TreePath, convert it to a PathBuf.  This is probably broken on
    /// non-utf8 file paths.
    pub fn tree_path_to_path(&self, tp: &TreePath) -> Result<PathBuf, anyhow::Error> {
        if let Some(ref dir) = self.model.dir {
            let mut stack = vec![];
            let mut ti = self.model.tree_store.get_iter(tp).unwrap();
            loop {
                let val = self.model.tree_store.get_value(&ti, 0);
                let s: String = val.get()?.unwrap();
                stack.push(s);

                if let Some(parent_ti) = self.model.tree_store.iter_parent(&ti) {
                    ti = parent_ti;
                } else {
                    break;
                }
            }
            let mut pb = PathBuf::from(dir);
            for s in stack.iter().rev() {
                pb.push(s);
            }
            Ok(pb)
        } else {
            bail!("no directory opened")
        }
    }

    pub fn row_activated(&mut self, tp: TreePath, _: TreeViewColumn) -> Result<(), anyhow::Error> {
        let path = self.tree_path_to_path(&tp)?;
        self.model
            .parent_relm
            .stream()
            .emit(crate::Msg::OpenPath(path));
        Ok(())
    }
}
