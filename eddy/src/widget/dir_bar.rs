use gtk::prelude::*;
use gtk::{
    Adjustment, CellRendererText, ListStore, ScrolledWindow, TreeStore, TreeView, TreeViewColumn,
};
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::Orientation::Horizontal;
use gtk::TreeModelSort;
use relm::{Relm, Widget};
use relm_derive::{widget, Msg};

pub struct Model {
    parent_relm: Relm<crate::Win>,
    tree_store: TreeStore,
    tree_model_sort: TreeModelSort,
}

#[derive(Msg)]
pub enum Msg {}

#[widget]
impl Widget for DirBar {
    fn model(parent_relm: Relm<crate::Win>) -> Model {
        let tree_store = TreeStore::new(&[String::static_type()]);
        Model {
            parent_relm,
            tree_store: tree_store.clone(),
            tree_model_sort: TreeModelSort::new(&tree_store),
        }
    }

    fn update(&mut self, event: Msg) {
        match event {}
    }

    view! {
        gtk::ScrolledWindow{
            gtk::TreeView {
                model: Some(&self.model.tree_model_sort),
            },
        }
    }
}

// pub struct DirBar {
//     dir: Option<PathBuf>,
//     scrolled_window: ScrolledWindow,
//     tree_view: TreeView,
// }

// impl Component for DirBar {
//     fn root_widget(&self) -> Widget {
//         self.scrolled_window.clone().upcast::<Widget>()
//     }
// }

// fn create_and_fill_model2() -> ListStore {
//     // Creation of a model with two rows.
//     let model = ListStore::new(&[u32::static_type(), String::static_type()]);

//     // Filling up the tree view.
//     let entries = &["Michel", "Sara", "Liam", "Zelda", "Neo", "Octopus master"];
//     for (i, entry) in entries.iter().enumerate() {
//         model.insert_with_values(None, &[0, 1], &[&(i as u32 + 1), &entry]);
//     }
//     model
// }

// impl DirBar {
//     pub fn new(dir: Option<&Path>, ctrl: ControllerRef) -> CompId {
//         let column = TreeViewColumn::new();
//         let cell = CellRendererText::new();
//         column.pack_start(&cell, true);
//         column.add_attribute(&cell, "text", 0);

//         let tree_view = TreeView::new();
//         tree_view.append_column(&column);
//         // tree_view.set_model(Some(&create_and_fill_model()));

//         tree_view.set_size_request(10, 10);
//         tree_view.show_all();

//         let scrolled_window = ScrolledWindow::new::<Adjustment, Adjustment>(None, None);
//         scrolled_window.add(&tree_view);
//         scrolled_window.set_size_request(10, 10);

//         let db = DirBar {
//             dir: dir.map(|p| p.to_path_buf()),
//             scrolled_window,
//             tree_view,
//         };
//         db.tree_view.set_model(Some(&db.create_and_fill_model()));
//         ctrl.borrow_mut().register(Box::new(db))
//     }

//     pub fn set_dir(&mut self, dir: Option<&Path>) {
//         self.dir = dir.map(|p| p.to_path_buf());
//         println!("set dir {:?}", dir);
//         self.tree_view
//             .set_model(Some(&self.create_and_fill_model()));
//     }

//     fn create_and_fill_model(&self) -> TreeStore {
//         use walkdir::WalkDir;

//         // Creation of a model with two rows.
//         let model = TreeStore::new(&[String::static_type()]);
//         if let Some(ref dir) = self.dir {
//             for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
//                 // println!("{}", entry.path().display());
//                 model.insert_with_values(
//                     None,
//                     None,
//                     &[0],
//                     &[&entry.path().to_string_lossy().as_ref()],
//                 );
//             }
//         }

//         let mut parent = None;

//         // Filling up the tree view.
//         let entries = &["Michel", "Sara", "Liam", "Zelda", "Neo", "Octopus master"];
//         for (i, entry) in entries.iter().enumerate() {
//             parent = Some(
//                 model
//                     .insert_with_values(parent.as_ref(), None, &[0], &[&entry])
//                     .clone(),
//             );
//         }
//         model
//     }
// }
