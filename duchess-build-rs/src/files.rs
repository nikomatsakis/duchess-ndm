use std::path::{Path, PathBuf};

use walkdir::WalkDir;

pub(crate) struct File {
    pub(crate) path: PathBuf,
    pub(crate) contents: String,
}

pub fn rs_files(path: &Path) -> impl Iterator<Item = anyhow::Result<File>> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| -> Option<anyhow::Result<File>> {
            match entry {
                Ok(entry) => {
                    if entry.path().extension().map_or(false, |e| e == "rs") {
                        Some(Ok(File {
                            path: entry.path().to_path_buf(),
                            contents: match std::fs::read_to_string(entry.path()) {
                                Ok(s) => s,
                                Err(err) => return Some(Err(err.into())),
                            },
                        }))
                    } else {
                        None
                    }
                }

                Err(err) => Some(Err(err.into())),
            }
        })
}
