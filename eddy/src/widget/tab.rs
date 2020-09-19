use gtk::prelude::*;
use gtk::Orientation::Horizontal;
use relm::{Relm, Widget};
use relm_derive::{widget, Msg};
use std::path::{Path, PathBuf};

pub struct Model {
    parent_relm: Relm<crate::Win>,
    file_name: Option<PathBuf>,
    pristine: bool,
    display_name: String,
    view_id: usize,
}

#[derive(Msg)]
pub enum Msg {
    Close,
    Pristine(bool),
}

#[widget]
impl Widget for Tab {
    fn model(
        (parent_relm, view_id, file_name): (Relm<crate::Win>, usize, Option<PathBuf>),
    ) -> Model {
        Model {
            parent_relm,
            display_name: display_name(true, file_name.clone()),
            file_name,
            pristine: true,
            view_id,
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::Close => self
                .model
                .parent_relm
                .stream()
                .emit(crate::Msg::CloseView(self.model.view_id.to_string())),
            Msg::Pristine(pristine) => {
                self.model.pristine = pristine;
                self.model.display_name =
                    display_name(self.model.pristine, self.model.file_name.clone());
            }
        }
    }

    view! {
        gtk::Box {
            orientation: Horizontal,
            gtk::Label {
                widget_name: "label",
                text: &self.model.display_name.to_string(),
            },
            gtk::Button {
                label: "X",
                clicked => Msg::Close,
            },
        }
    }
}

fn display_name(pristine: bool, file_name: Option<PathBuf>) -> String {
    let pristine_mark = if pristine { "" } else { "*" };
    let name = file_name
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or("Untitled".to_string());
    format!("{}{}", pristine_mark, name)
}
