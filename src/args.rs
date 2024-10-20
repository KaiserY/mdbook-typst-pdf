use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// Common arguments of compile, watch, and query.
#[derive(Debug, Clone)]
pub struct SharedArgs {
  /// Path to input Typst file. Use `-` to read input from stdin
  pub input: Input,

  /// Configures the project root (for absolute paths)
  pub root: Option<PathBuf>,

  /// Add a string key-value pair visible through `sys.inputs`
  pub inputs: Vec<(String, String)>,

  /// Common font arguments
  pub font_args: FontArgs,

  /// The document's creation date formatted as a UNIX timestamp.
  ///
  /// For more information, see <https://reproducible-builds.org/specs/source-date-epoch/>.
  pub creation_timestamp: Option<DateTime<Utc>>,

  /// Arguments related to storage of packages in the system
  pub package_storage_args: PackageStorageArgs,

  pub output: PathBuf,
}

/// Which format to use for diagnostics.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiagnosticFormat {
  Human,
  Short,
}

/// An input that is either stdin or a real path.
#[derive(Debug, Clone)]
pub enum Input {
  /// Stdin, represented by `-`.
  #[allow(dead_code)]
  Stdin,
  /// A non-empty path.
  Path(PathBuf),
}

/// Arguments related to where packages are stored in the system.
#[derive(Debug, Clone)]
pub struct PackageStorageArgs {
  /// Custom path to local packages, defaults to system-dependent location
  pub package_path: Option<PathBuf>,

  /// Custom path to package cache, defaults to system-dependent location
  pub package_cache_path: Option<PathBuf>,
}

/// Common arguments to customize available fonts
#[derive(Debug, Clone)]
pub struct FontArgs {
  /// Adds additional directories that are recursively searched for fonts
  ///
  /// If multiple paths are specified, they are separated by the system's path
  /// separator (`:` on Unix-like systems and `;` on Windows).
  pub font_paths: Vec<PathBuf>,

  /// Ensures system fonts won't be searched, unless explicitly included via
  /// `--font-path`
  pub ignore_system_fonts: bool,
}
