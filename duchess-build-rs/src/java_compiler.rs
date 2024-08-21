use anyhow::Context;
use duchess_reflect::{
    class_info::{DotId, Id},
    config::Configuration,
};
use heck::ToShoutySnakeCase;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

use crate::code_writer::CodeWriter;

pub struct JavaCompiler {
    /// Configuration for running javac
    configuration: Configuration,

    /// Where to put Java files
    source_path: PathBuf,

    /// Where to put class files
    output_path: PathBuf,

    /// If true, the Rust code should use an external class file
    external: bool,

    /// Guard that will delete temporary directory when done (if needed)
    #[allow(dead_code)]
    temp_dir: Option<TempDir>,

    /// Output directory for final Rust results
    out_dir: PathBuf,
}

pub struct JavaFile {
    pub class_name: DotId,
    pub java_path: PathBuf,
    pub class_path: PathBuf,
    pub rs_path: PathBuf,
}

impl JavaCompiler {
    pub fn new(
        configuration: &Configuration,
        temporary_dir: Option<&PathBuf>,
        output_dir: Option<&PathBuf>,
    ) -> anyhow::Result<Self> {
        let (temp_dir, temp_dir_path);
        match temporary_dir {
            Some(d) => {
                temp_dir_path = d.clone();
                temp_dir = None;
            }
            None => {
                let d = tempfile::TempDir::new()?;
                temp_dir_path = d.path().to_path_buf();
                temp_dir = Some(d);
            }
        }

        let source_path = temp_dir_path.join("src");
        let output_path = match output_dir {
            Some(d) => d.clone(),
            None => temp_dir_path.join("class"),
        };

        Ok(Self {
            configuration: configuration.clone(),
            temp_dir,
            source_path,
            output_path,
            external: output_dir.is_some(),
            out_dir: std::env::var("OUT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target")),
        })
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub fn java_file(&self, class_name: &DotId) -> JavaFile {
        let (package_ids, class_id) = class_name.split();
        let java_path = self
            .make_package_dir(&self.source_path, package_ids)
            .join(&class_id[..])
            .with_extension("java");
        let class_path = self
            .make_package_dir(&self.output_path, package_ids)
            .join(&class_id[..])
            .with_extension("class");
        let rs_path = self.out_dir.join(format!("{}.rs", class_id));
        JavaFile {
            class_name: class_name.clone(),
            java_path,
            class_path,
            rs_path,
        }
    }

    fn make_package_dir(&self, path: &Path, package: &[Id]) -> PathBuf {
        let mut path = path.to_path_buf();
        for id in package {
            path.push(&id[..]);
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    pub fn compile(&self, java_file: &JavaFile) -> anyhow::Result<()> {
        let exit_status = Command::new(self.configuration.bin_path("javac"))
            .arg("-cp")
            .arg(self.configuration.classpath().unwrap_or_default())
            .arg("-d")
            .arg(&self.output_path)
            .arg(&java_file.java_path)
            .status()
            .with_context(|| format!("compiling `{}`", java_file.java_path.display()))?;

        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "exit status {} returned compiling `{}`",
                exit_status,
                java_file.java_path.display()
            ))
        }
    }

    pub fn compile_to_rs_file(&self, java_file: &JavaFile) -> anyhow::Result<()> {
        self.compile(java_file)?;

        let constant_name = java_file
            .class_name
            .class_name()
            .replace("$", "__")
            .to_shouty_snake_case();

        {
            let mut rs_file = std::fs::File::create(&java_file.rs_path)?;
            let mut cw = CodeWriter::new(&mut rs_file);

            if self.external {
                write!(cw, "pub const {constant_name}: duchess::plumbing::ClassDefinition = duchess::plumbing::ClassDefinition::new_external(")?;
                write!(cw, "{:?},", java_file.class_name.to_string())?;
                write!(
                    cw,
                    "unsafe {{ ::core::ffi::CStr::from_bytes_with_nul_unchecked(&["
                )?;
                for b in java_file.class_name.to_jni_name().as_bytes() {
                    write!(cw, "{}_u8,", b)?;
                }
                write!(cw, "0_u8,")?;
                write!(cw, "]) }},")?;
                write!(cw, ");")?;
            } else {
                let class_bytes = java_file.compiled_bytes()?;
                write!(cw, "pub const {constant_name}: duchess::plumbing::ClassDefinition = duchess::plumbing::ClassDefinition::new(")?;
                write!(cw, "{:?},", java_file.class_name.to_string())?;
                write!(cw, "{:?},", java_file.class_name.to_jni_name())?;
                write!(cw, "&[")?;
                for byte in class_bytes {
                    write!(cw, "{}_i8,", byte as i8)?;
                }
                write!(cw, "],")?;
                write!(cw, ");")?;
            }
        }

        Ok(())
    }
}

impl JavaFile {
    pub fn src_writer(&self) -> anyhow::Result<std::fs::File> {
        std::fs::File::create(&self.java_path)
            .with_context(|| format!("writing to `{}`", self.java_path.display()))
    }

    pub fn compiled_bytes(&self) -> anyhow::Result<Vec<u8>> {
        std::fs::read(&self.class_path)
            .with_context(|| format!("reading from `{}`", self.class_path.display()))
    }
}
