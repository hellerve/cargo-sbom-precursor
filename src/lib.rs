//! Types for the SBOM precursor files Cargo writes with `-Z sbom`.
//!
//! ```no_run
//! let precursor = cargo_sbom_precursor::Precursor::from_path("target/debug/app.cargo-sbom.json")?;
//! println!("{} was built for {}", precursor.root_crate().id, precursor.target);
//! for dep in &precursor.root_crate().dependencies {
//!     println!("  {} ({:?})", precursor.crates[dep.index].id, dep.kind);
//! }
//! # Ok::<(), cargo_sbom_precursor::Error>(())
//! ```

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The precursor format version for this crate
pub const FORMAT_VERSION: u32 = 1;

/// Suffix Cargo appends to an artifact's path to name its precursor file.
pub const FILE_SUFFIX: &str = ".cargo-sbom.json";

/// The contents of a precursor file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Precursor {
    /// Format version, currently always [`FORMAT_VERSION`].
    pub version: u32,
    /// Index into [`crates`](Self::crates) of the crate that produced the artifact.
    pub root: usize,
    /// Every crate compiled for the artifact.
    pub crates: Vec<Crate>,
    /// The compiler used for the build.
    pub rustc: Rustc,
    /// Target triple the artifact was built for.
    pub target: String,
}

/// A compiled crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Crate {
    /// Fully qualified package ID spec, e.g.
    /// `registry+https://github.com/rust-lang/crates.io-index#serde@1.0.228`.
    pub id: String,
    /// Target kinds, e.g. `bin`, `lib`, `rlib`, `proc-macro` or `custom-build`.
    pub kind: Vec<String>,
    /// Features enabled for this compilation.
    pub features: Vec<String>,
    /// Direct dependencies of this crate.
    pub dependencies: Vec<Dependency>,
}

/// An edge in the dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Dependency {
    /// Index into [`Precursor::crates`].
    pub index: usize,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DependencyKind {
    /// Linked into the artifact produced by the depending crate.
    Normal,
    /// Only needed to build the depending crate, e.g. a build script or proc-macro.
    Build,
    /// A kind this version of the crate does not know about.
    #[serde(other)]
    Unknown,
}

/// The compiler used for the build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Rustc {
    /// e.g. `1.86.0-nightly`
    pub version: String,
    /// `RUSTC_WRAPPER`, if set.
    pub wrapper: Option<PathBuf>,
    /// `RUSTC_WORKSPACE_WRAPPER`, if set.
    pub workspace_wrapper: Option<PathBuf>,
    /// `None` for compilers built without git information.
    pub commit_hash: Option<String>,
    /// Host target triple.
    pub host: String,
    /// Output of `rustc -vV`.
    pub verbose_version: String,
}

impl Precursor {
    /// Parses a precursor and checks its format version.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let precursor: Precursor = serde_json::from_slice(bytes).map_err(Error::Json)?;
        if precursor.version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(precursor.version));
        }
        if precursor.root >= precursor.crates.len() {
            return Err(Error::InvalidIndex(precursor.root));
        }
        if let Some(dep) = precursor
            .crates
            .iter()
            .flat_map(|krate| &krate.dependencies)
            .find(|dep| dep.index >= precursor.crates.len())
        {
            return Err(Error::InvalidIndex(dep.index));
        }
        Ok(precursor)
    }

    /// Reads and parses a precursor file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        Self::from_slice(&bytes)
    }

    /// The crate that produced the artifact.
    pub fn root_crate(&self) -> &Crate {
        &self.crates[self.root]
    }
}

/// The path of the precursor Cargo writes for an artifact.
pub fn path_for_artifact(artifact: impl AsRef<Path>) -> PathBuf {
    let mut path = artifact.as_ref().as_os_str().to_owned();
    path.push(FILE_SUFFIX);
    PathBuf::from(path)
}

/// The precursor paths Cargo passes to `rustc`.
pub fn paths_from_env() -> Vec<PathBuf> {
    paths_from_env_value(std::env::var_os("CARGO_SBOM_PATH").as_deref())
}

fn paths_from_env_value(value: Option<&OsStr>) -> Vec<PathBuf> {
    match value {
        Some(value) if !value.is_empty() => std::env::split_paths(value).collect(),
        _ => Vec::new(),
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// The precursor has a format version this crate does not understand.
    UnsupportedVersion(u32),
    /// An index into [`Precursor::crates`] is out of bounds.
    InvalidIndex(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(_) => write!(f, "could not read SBOM precursor"),
            Error::Json(_) => write!(f, "could not parse SBOM precursor"),
            Error::UnsupportedVersion(version) => write!(
                f,
                "unsupported SBOM precursor version {version}, expected {FORMAT_VERSION}"
            ),
            Error::InvalidIndex(index) => {
                write!(
                    f,
                    "SBOM precursor refers to crate {index}, which does not exist"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Json(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_path() {
        assert_eq!(
            path_for_artifact("target/debug/libfoo.rlib"),
            PathBuf::from("target/debug/libfoo.rlib.cargo-sbom.json")
        );
    }

    #[test]
    fn env_paths() {
        assert!(paths_from_env_value(None).is_empty());
        assert!(paths_from_env_value(Some(OsStr::new(""))).is_empty());
        let joined = std::env::join_paths(["a.cargo-sbom.json", "b.cargo-sbom.json"]).unwrap();
        assert_eq!(
            paths_from_env_value(Some(&joined)),
            vec![
                PathBuf::from("a.cargo-sbom.json"),
                PathBuf::from("b.cargo-sbom.json")
            ]
        );
    }
}
