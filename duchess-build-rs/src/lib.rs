use std::path::{Path, PathBuf};

use java_compiler::JavaCompiler;

mod code_writer;
mod files;
mod impl_java_trait;
mod java_compiler;
mod re;
mod shim_writer;

pub struct DuchessBuildRs {
    src_path: PathBuf,
    in_cargo: bool,
}

impl Default for DuchessBuildRs {
    fn default() -> Self {
        DuchessBuildRs {
            src_path: PathBuf::from("."),
            in_cargo: std::env::var("CARGO").is_ok() && std::env::var("OUT_DIR").is_ok(),
        }
    }
}

impl DuchessBuildRs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn src_path(mut self, src_path: PathBuf) -> Self {
        self.src_path = src_path;
        self
    }

    fn emit_rerun_if_changed(&self, path: &Path) {
        if self.in_cargo {
            cargo_emit::rerun_if_changed!(path.display());
        }
    }

    pub fn execute(self) -> anyhow::Result<()> {
        let compiler = &JavaCompiler::new()?;
        for rs_file in files::rs_files(&self.src_path) {
            let rs_file = rs_file?;

            self.emit_rerun_if_changed(&rs_file.path);

            for capture in re::impl_java_interface().captures_iter(&rs_file.contents) {
                let std::ops::Range { start, end: _ } = capture.get(0).unwrap().range();
                impl_java_trait::process_impl(compiler, &rs_file, start)?;
            }
        }
        Ok(())
    }
}
