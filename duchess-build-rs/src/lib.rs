use std::path::{Path, PathBuf};

use java_compiler::JavaCompiler;

mod code_writer;
mod files;
mod impl_java_trait;
mod java_compiler;
mod re;
mod shim_writer;

pub use duchess_reflect::config::Configuration;

pub struct DuchessBuildRs {
    configuration: Configuration,
    src_path: PathBuf,
    in_cargo: bool,
    temporary_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
}

impl Default for DuchessBuildRs {
    fn default() -> Self {
        DuchessBuildRs {
            configuration: Configuration::default(),
            src_path: PathBuf::from("."),
            in_cargo: std::env::var("CARGO").is_ok() && std::env::var("OUT_DIR").is_ok(),
            temporary_dir: None,
            output_dir: None,
        }
    }
}

impl DuchessBuildRs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Customize the JDK configuration (e.g., CLASSPATH, etc).
    pub fn with_configuration(mut self, configuration: Configuration) -> Self {
        self.configuration = configuration;
        self
    }

    /// Where to scan for Rust source files that will be preprocessed
    pub fn with_src_path(mut self, src_path: PathBuf) -> Self {
        self.src_path = src_path;
        self
    }

    /// Where to store temporary files (generated java, class files that are not being exported).
    /// If unset, a fresh temporary directory is created that will be wiped up later.
    pub fn with_output_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.output_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Where to store temporary files (generated java, class files that are not being exported).
    /// If unset, a fresh temporary directory is created that will be wiped up later.
    pub fn with_temporary_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.temporary_dir = Some(path.as_ref().to_path_buf());
        self
    }

    fn emit_rerun_if_changed(&self, path: &Path) {
        if self.in_cargo {
            cargo_emit::rerun_if_changed!(path.display());
        }
    }

    /// Execute the duchess build-rs step, preprocessing Rust files.
    /// The precise actions this takes will depend on the annotations found within the source directory.
    pub fn execute(self) -> anyhow::Result<()> {
        let compiler = &JavaCompiler::new(
            &self.configuration,
            self.temporary_dir.as_ref(),
            self.output_dir.as_ref(),
        )?;
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
