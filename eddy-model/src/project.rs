use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::ProjectId;

#[derive(Debug, Clone)]
pub struct Project {
    pub name: String,
    pub dir: PathBuf,
    pub files: Option<BTreeMap<OsString, FileNode>>,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Option<BTreeMap<OsString, FileNode>>,
}

impl FileNode {
    pub fn new(path: PathBuf, is_dir: bool) -> Self {
        Self {
            path,
            is_dir,
            children: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectPath {
    pub id: ProjectId,
    /// A relative path from the project root
    pub path: PathBuf,
}

impl Project {
    pub fn new(name: &str, dir: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            dir,
            files: None,
        }
    }

    /// Given a relative path under a project, return a list of FileNodes
    pub fn child_file_nodes(&self, rel_path: PathBuf) -> Option<&BTreeMap<OsString, FileNode>> {
        let mut files = self.files.as_ref();
        for comp in &rel_path {
            files = files
                .and_then(|fs| fs.get(comp))
                .and_then(|fnode| fnode.children.as_ref())
        }
        files
    }
}
