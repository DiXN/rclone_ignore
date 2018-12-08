use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Op {
  CREATE,
  WRITE,
  RENAME,
  REMOVE,
  CHMOD
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PathOp {
  pub old_path: PathBuf,
  pub path: PathBuf,
  pub op: Op
}

impl PathOp {
  pub fn new(old_path: &Path, path: &Path, op: Op) -> PathOp {
    PathOp {
      old_path: old_path.to_path_buf(),
      path: path.to_path_buf(),
      op: op,
    }
  }
}