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

impl File {
    /// Return a string that can be used as a slug for error messages.
    pub fn slug(&self, offset: usize) -> String {
        let line_num = self.contents[..offset].lines().count();
        let column_num = 1 + self.contents[..offset]
            .rfind('\n')
            .map_or(offset, |i| offset - i - 1);
        format!(
            "{path}:{line_num}:{column_num}:",
            path = self.path.display(),
        )
    }

    /// Returns a chunk of rust code starting at `offset`
    /// and extending until the end of the current token tree
    /// or file, whichever comes first.
    ///
    /// This is used when we are preprocessing and we find
    /// some kind of macro invocation. We want to grab all
    /// the text that may be part of it and pass it into `syn`.
    pub fn rust_slice_from(&self, offset: usize) -> &str {
        let mut counter = 0;
        let terminator = self.contents[offset..].char_indices().find(|&(_, c)| {
            if c == '{' || c == '[' || c == '(' {
                counter += 1;
            } else if c == '}' || c == ']' || c == ')' {
                if counter == 0 {
                    return true;
                }

                counter -= 1;
            }

            false
        });
        match terminator {
            Some((i, _)) => &self.contents[offset..offset + i],
            None => &self.contents[offset..],
        }
    }
}
