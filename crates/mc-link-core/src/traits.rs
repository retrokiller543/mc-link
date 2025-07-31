use std::path::{Path, PathBuf};

pub trait PathExt {
    fn to_slash_lossy(&self) -> String;
}

impl PathExt for Path {
    fn to_slash_lossy(&self) -> String {
        self.to_string_lossy().replace('\\', "/")
    }
}

impl PathExt for PathBuf {
    fn to_slash_lossy(&self) -> String {
        self.as_path().to_slash_lossy()
    }
}