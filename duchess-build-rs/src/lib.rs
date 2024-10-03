use std::path::PathBuf;

use java_compiler::JavaCompiler;

mod code_writer;
mod files;
mod impl_java_trait;
mod java_compiler;
mod re;
mod shim_writer;

/// Build Rs configuration for duchess.
/// To use duchess you must invoke [`DuchessBuildRs::execute`][].
///
/// # Example
///
/// The simplest build.rs is as follows.
///
/// ```rust
/// use duchess_build_rs::DuchessBuildRs;
///
/// fn main() -> anyhow::Result<()> {
///     DuchessBuildRs::new().execute()?;
/// }
/// ```
pub struct DuchessBuildRs {
    src_path: PathBuf,
}

impl Default for DuchessBuildRs {
    fn default() -> Self {
        DuchessBuildRs {
            src_path: PathBuf::from("."),
        }
    }
}

impl DuchessBuildRs {
    /// Create a new DuchessBuildRs instance.
    /// Equivalent to `DuchessBuildRs::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the path where Rust sources are found.
    /// The default is `.`.
    /// We will automatically search all subdirectories for `.rs` files.
    pub fn src_path(mut self, src_path: PathBuf) -> Self {
        self.src_path = src_path;
        self
    }

    /// Execute the duchess `build.rs` processing.
    ///
    /// Detects uses of duchess build macros and derives
    /// and generates necessary support files in the `OUT_DIR` side.
    pub fn execute(self) -> anyhow::Result<()> {
        let compiler = &JavaCompiler::new()?;
        for rs_file in files::rs_files(&self.src_path) {
            let rs_file = rs_file?;
            for capture in re::impl_java_interface().captures_iter(&rs_file.contents) {
                let std::ops::Range { start, end: _ } = capture.get(0).unwrap().range();
                impl_java_trait::process_impl(compiler, &rs_file, start)?;
            }
        }
        Ok(())
    }
}
