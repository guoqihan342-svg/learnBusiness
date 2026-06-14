use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::AppConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    index_dir: PathBuf,
}

impl Workspace {
    pub fn init(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let index_dir = root.join(".agent-index");
        let workspace = Self { root, index_dir };
        workspace.create_layout()?;
        workspace.write_default_config()?;
        Ok(workspace)
    }

    pub fn open(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let index_dir = root.join(".agent-index");
        Self { root, index_dir }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    pub fn metadata_db_path(&self) -> PathBuf {
        self.index_dir.join("metadata.sqlite")
    }

    pub fn ai_cache_dir(&self) -> PathBuf {
        self.index_dir.join("cache").join("ai")
    }

    pub fn config_path(&self) -> PathBuf {
        self.index_dir.join("config.toml")
    }

    fn create_layout(&self) -> Result<()> {
        for dir in [
            self.index_dir.clone(),
            self.index_dir.join("fulltext"),
            self.index_dir.join("vectors"),
            self.index_dir.join("artifacts").join("images"),
            self.index_dir.join("artifacts").join("pages"),
            self.index_dir.join("artifacts").join("thumbnails"),
            self.index_dir.join("cache").join("ai"),
            self.index_dir.join("cache").join("extraction"),
            self.index_dir.join("logs"),
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    fn write_default_config(&self) -> Result<()> {
        let path = self.config_path();
        if !path.exists() {
            fs::write(path, AppConfig::default().to_toml_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_init_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        Workspace::init(temp.path()).unwrap();
        Workspace::init(temp.path()).unwrap();
        assert!(temp.path().join(".agent-index/config.toml").exists());
    }
}
