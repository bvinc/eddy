use anyhow::bail;
use eddy_model::{Model, Window};
use gflux::{Component, ComponentCtx};
use gio::subclass::prelude::*;
use glib::subclass::prelude::*;
use glib::{clone, Propagation};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{prelude::*, Label, MultiSelection, StringObject, TreeListModel};
use log::*;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::objects::file_node::FileNode;
use crate::objects::project_list::ProjectGList;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DirBarComponent {
    list_view: gtk::ListView,
}

impl Component for DirBarComponent {
    type GlobalModel = Model;
    type Model = Window;
    type Widget = gtk::ListView;
    type Params = ();

    fn widget(&self) -> Self::Widget {
        self.list_view.clone()
    }

    fn build(ctx: ComponentCtx<Self>, _params: ()) -> Self {
        // let proj_list = ProjectGList::new(ctx);
        // let proj_list: Vec<glib::Object> = Vec::new();
        let proj_list = gio::ListStore::new::<FileNode>();

        ctx.with_model(|m| {
            for (proj_id, p) in &m.projects {
                let f = FileNode::new(&p.name, *proj_id, PathBuf::new(), true);
                proj_list.append(f.upcast_ref::<glib::Object>());
            }
        });

        // let list_model = gio::ListStore::new::<gtk::StringObject>();
        // list_model.append(&gtk::StringObject::new("hello"));
        let tl_model = gtk::TreeListModel::new(proj_list, false, false, move |obj| {
            dbg!(obj);
            let file_node = obj.downcast_ref::<FileNode>()?;
            // ctx.with_model_mut(|m| {
            //     let abs_dir = m
            //         .projects
            //         .get(&file_node.proj_id())
            //         .map(|p| p.dir.clone())
            //         .map(|proj_dir| proj_dir.join(&file_node.path()));
            //     if let Some(abs_dir) = abs_dir {
            //         m.backend.list_files(
            //             &abs_dir,
            //             Box::new(|win, entries| {
            //                 // dbg!(entries);
            //             }),
            //         );
            //     }
            // });

            let list_store = gio::ListStore::new::<FileNode>();
            let _: Option<()> = ctx.with_model(|m| {
                let p = m.projects.get(&file_node.proj_id())?;
                dbg!(p.child_file_nodes(file_node.path()));
                for (name, cfn) in p.child_file_nodes(file_node.path())? {
                    dbg!(name, cfn);
                    let f = FileNode::new(
                        &name.to_string_lossy(),
                        file_node.proj_id(),
                        file_node.path().join(name),
                        cfn.is_dir,
                    );
                    list_store.append(f.upcast_ref::<glib::Object>());
                }
                None
            });

            // list_store.append(&gtk::StringObject::new("hello2"));
            Some(list_store.into())
        });
        let multi_selection = gtk::MultiSelection::new(Some(tl_model));
        let list_item_factory = gtk::SignalListItemFactory::new();
        list_item_factory.connect_setup(|_, list_item| {
            || -> Option<()> {
                println!("setup");
                let list_item = list_item.downcast_ref::<gtk::ListItem>()?;
                let label = Label::new(None);
                let expander = gtk::TreeExpander::new();
                expander.set_child(Some(&label));
                list_item.set_child(Some(&expander));
                Some(())
            }();
        });
        list_item_factory.connect_bind(|_, list_item| {
            println!("bind");
            || -> Option<()> {
                let list_item = list_item.downcast_ref::<gtk::ListItem>()?;
                let expander = list_item.child()?.downcast::<gtk::TreeExpander>().ok()?;
                let label = expander.child()?.downcast::<gtk::Label>().ok()?;

                let row = list_item.item()?.downcast::<gtk::TreeListRow>().ok()?;
                let file_node = row.item()?.downcast::<FileNode>().ok()?;

                expander.set_list_row(Some(&row));
                expander.set_hide_expander(!dbg!(file_node.is_dir()));
                label.set_label(file_node.proj_name().as_str());
                Some(())
            }();
            // if let (Some(label), Some(str_obj)) = (
            //     dbg!(list_item
            //         .child()
            //         .and_then(|w| w.downcast::<gtk::TreeExpander>().ok())
            //         .and_then(|te| te.child())
            //         .and_then(|w| w.downcast::<gtk::Label>().ok())),
            //     dbg!(list_item
            //         .item()
            //         .and_then(|row| row.downcast::<gtk::TreeListRow>().ok())
            //         .and_then(|row| row.item())
            //         .and_then(|row| row.downcast::<gtk::StringObject>().ok())),
            // ) {
            //     label.set_label(str_obj.string().as_str());
            // }
            // list_item.set_child(Some(&Label::new(Some("blah"))));
        });
        list_item_factory.connect_setup(|_, _| println!("setup"));
        let list_view = gtk::ListView::new(Some(multi_selection), Some(list_item_factory));

        let column0 = gtk::TreeViewColumn::new();
        let cell0 = gtk::CellRendererText::new();
        column0.pack_start(&cell0, true);
        column0.add_attribute(&cell0, "text", 0);
        /*
                // let tree_view = TreeView::new();
                tree_view.set_model(Some(&tree_store));
                tree_view.set_headers_visible(false);

                tree_view.append_column(&column0);

                tree_view.connect_test_expand_row(
                    clone!(@strong ctx, @strong tree_store => move |_tv, ti, tp| {
                        dbg!("handle_test_expand_row");
                        let dir = ctx.with_model(|win| win.dir.clone());

                        if let Ok(path) = tree_path_to_path(Some(&dir), &tree_store, tp) {
                            if let Err(e) = refresh_dir(&ctx, &tree_store, Some(ti), &path) {
                                warn!("{}", e);
                            }
                        }
                        Propagation::Proceed
                    }),
                );

                tree_view.connect_test_collapse_row(|_tv, _ti, _tp| Propagation::Proceed);

                tree_view.connect_row_activated(
                    clone!(@strong ctx, @strong tree_store => move |tv, tp, _tvc| {
                        let dir = ctx.with_model(|win| win.dir.clone());
                        if let Some(ref ti) = tree_store.iter(tp) {
                            if tree_store.iter_has_child(ti) {
                                if !tv.row_expanded(tp) {
                                    tv.expand_row(tp, false);
                                } else {
                                    tv.collapse_row(tp);
                                }
                                return;
                            }
                        } else {
                            dbg!("invalid path");
                            return;
                        }
                        match tree_path_to_path(Some(&dir), &tree_store, tp) {
                            Ok(path) => {
                                if let Ok(path) = path.canonicalize() {
                                    dbg!(&path);
                                    ctx.with_model_mut(|win| win.new_view(Some(&path)));
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
                let _ = refresh_dir(&ctx, &tree_store, None, &dir);
        */
        Self { list_view }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        || -> Option<()> {
            let tl_model = self
                .list_view
                .model()?
                .downcast_ref::<MultiSelection>()?
                .model()?
                .downcast::<TreeListModel>()
                .ok()?;
            let list_store = tl_model.model().downcast::<gio::ListStore>().ok()?;
            // dbg!(tl_model.item(0)?.downcast::<gtk::TreeListRow>().ok()?);
            // dbg!(tl_model.row(0));
            // dbg!(tl_model.child_row(0));
            let item0 = list_store.item(0);
            // dbg!(item0);
            Some(())
        }();
        // let list_store = self
        //     .list_view
        //     .model()
        //     .unwrap()
        //     .downcast_ref::<MultiSelection>()
        //     .unwrap()
        //     .model()
        //     .unwrap()
        //     .downcast::<TreeListModel>()
        //     .unwrap()
        //     .model()
        //     .downcast::<gio::ListStore>()
        //     .unwrap();
        // dbg!(&list_store);
        // list_store.append(&gtk::StringObject::new("asdf"));
        ctx.rebuild_children()
    }
}

/// Given a path in the tree, clear it of its children, and re-read the
/// files from the disk.
pub fn refresh_dir(
    ctx: &ComponentCtx<DirBarComponent>,
    tree_store: &gtk::TreeStore,
    ti: Option<&gtk::TreeIter>,
    path: &Path,
) -> Result<(), anyhow::Error> {
    dbg!("refresh_dir");
    dbg!("clearing children");
    clear_tree_iter_children(tree_store, ti);

    let mut files = vec![];

    dbg!(&path);
    ctx.with_model_mut(|win| {
        win.backend.list_files(
            path,
            Box::new(|win, files| {
                dbg!(files);
            }),
        )
    });

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
    }
}
