use std::{path::PathBuf, process::Command};

use anyhow::Context;

use crate::code_writer::{self, CodeWriter};

pub struct JavaCompiler {
    temp_dir: PathBuf,
    out_dir: PathBuf,
}

pub struct JavaFile {
    pub java_path: PathBuf,
    pub class_path: PathBuf,
    pub rs_path: PathBuf,
}

impl JavaCompiler {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            temp_dir: tempfile::TempDir::new()?.into_path(),
            out_dir: std::env::var("OUT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target")),
        })
    }

    fn src_dir(&self) -> PathBuf {
        self.temp_dir.join("src")
    }

    fn class_dir(&self) -> PathBuf {
        self.temp_dir.join("class")
    }

    pub fn java_file(&self, package: &str, class_name: &str) -> JavaFile {
        let java_path = self
            .make_package_dir(self.src_dir(), package)
            .join(class_name)
            .with_extension("java");
        let class_path = self
            .make_package_dir(self.class_dir(), package)
            .join(class_name)
            .with_extension("class");
        let rs_path = self.out_dir.join(format!("{}.rs", class_name));
        JavaFile {
            java_path,
            class_path,
            rs_path,
        }
    }

    fn make_package_dir(&self, mut path: PathBuf, package: &str) -> PathBuf {
        for part in package.split('.') {
            path.push(part);
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    pub fn compile(&self, java_file: &JavaFile) -> anyhow::Result<()> {
        let exit_status = Command::new("javac")
            .arg("-d")
            .arg(self.class_dir())
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

        let source_text = java_file.source_text()?;
        let class_bytes = java_file.compiled_bytes()?;

        {
            let mut rs_file = std::fs::File::create(&java_file.rs_path)?;
            let mut cw = CodeWriter::new(&mut rs_file);

            write!(cw, "pub const JAVA_SOURCE: &str = {source_text:?}")?;

            write!(cw, "pub const CLASS_BYTES: &[u8] = &[")?;
            for byte in class_bytes {
                write!(cw, "{},", byte)?;
            }
            write!(cw, "];")?;
        }

        Ok(())
    }
}

impl JavaFile {
    pub fn src_writer(&self) -> anyhow::Result<std::fs::File> {
        std::fs::File::create(&self.java_path)
            .with_context(|| format!("writing to `{}`", self.java_path.display()))
    }

    pub fn source_text(&self) -> anyhow::Result<String> {
        std::fs::read_to_string(&self.java_path)
            .with_context(|| format!("reading from `{}`", self.class_path.display()))
    }

    pub fn compiled_bytes(&self) -> anyhow::Result<Vec<u8>> {
        std::fs::read(&self.class_path)
            .with_context(|| format!("reading from `{}`", self.class_path.display()))
    }
}
