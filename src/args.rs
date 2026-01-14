// This module is imported both from the `typst-cli` crate itself
// and from its build script. In this module, you can only import from crates
// that are both runtime and build dependencies of this crate, or else
// Rust will give a confusing error message about a missing crate.

use std::fmt::{self, Display, Formatter};
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};

/// Compiles an input file into a supported output format.
#[derive(Debug, Clone)]
pub struct CompileCommand {
  /// Arguments for compilation.
  pub args: CompileArgs,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WatchCommand {
  /// Arguments for compilation.
  pub args: CompileArgs,
}

#[allow(dead_code)]
/// Arguments for compilation and watching.
#[derive(Debug, Clone)]
pub struct CompileArgs {
  /// Path to input Typst file. Use `-` to read input from stdin.
  pub input: Input,

  /// Path to output file (PDF, PNG, SVG, or HTML). Use `-` to write output to
  /// stdout.
  ///
  /// For output formats emitting one file per page (PNG & SVG), a page number
  /// template must be present if the source document renders to multiple
  /// pages. Use `{p}` for page numbers, `{0p}` for zero padded page numbers
  /// and `{t}` for page count. For example, `page-{0p}-of-{t}.png` creates
  /// `page-01-of-10.png`, `page-02-of-10.png`, and so on.
  pub output: Option<Output>,

  /// The format of the output file, inferred from the extension by default.
  pub format: Option<OutputFormat>,

  /// World arguments.
  pub world: WorldArgs,

  /// Which pages to export. When unspecified, all pages are exported.
  ///
  /// Pages to export are separated by commas, and can be either simple page
  /// numbers (e.g. '2,5' to export only pages 2 and 5) or page ranges (e.g.
  /// '2,3-6,8-' to export page 2, pages 3 to 6 (inclusive), page 8 and any
  /// pages after it).
  ///
  /// Page numbers are one-indexed and correspond to physical page numbers in
  /// the document (therefore not being affected by the document's page
  /// counter).
  pub pages: Option<Vec<Pages>>,

  /// One (or multiple comma-separated) PDF standards that Typst will enforce
  /// conformance with.
  pub pdf_standard: Vec<PdfStandard>,

  /// By default, even when not producing a `PDF/UA-1` document, a tagged PDF
  /// document is written to provide a baseline of accessibility. In some
  /// circumstances (for example when trying to reduce the size of a document)
  /// it can be desirable to disable tagged PDF.
  pub no_pdf_tags: bool,

  /// The PPI (pixels per inch) to use for PNG export.
  pub ppi: f32,

  /// File path to which a Makefile with the current compilation's
  /// dependencies will be written.
  pub make_deps: Option<PathBuf>,

  /// File path to which a list of current compilation's dependencies will be
  /// written. Use `-` to write to stdout.
  pub deps: Option<Output>,

  /// File format to use for dependencies.
  pub deps_format: DepsFormat,

  /// Processing arguments.
  pub process: ProcessArgs,

  /// Opens the output file with the default viewer or a specific program
  /// after compilation. Ignored if output is stdout.
  pub open: Option<Option<String>>,

  /// Produces performance timings of the compilation process. (experimental)
  ///
  /// The resulting JSON file can be loaded into a tracing tool such as
  /// https://ui.perfetto.dev. It does not contain any sensitive information
  /// apart from file names and line numbers.
  pub timings: Option<Option<PathBuf>>,
}

/// Arguments for the construction of a world. Shared by compile, watch, and
/// query.
#[derive(Debug, Clone)]
pub struct WorldArgs {
  /// Configures the project root (for absolute paths).
  pub root: Option<PathBuf>,

  /// Add a string key-value pair visible through `sys.inputs`.
  pub inputs: Vec<(String, String)>,

  /// Common font arguments.
  pub font: FontArgs,

  /// Arguments related to storage of packages in the system.
  pub package: PackageArgs,

  /// The document's creation date formatted as a UNIX timestamp.
  ///
  /// For more information, see <https://reproducible-builds.org/specs/source-date-epoch/>.
  pub creation_timestamp: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
/// Arguments for configuration the process of compilation itself.
#[derive(Debug, Clone)]
pub struct ProcessArgs {
  /// Number of parallel jobs spawned during compilation. Defaults to number
  /// of CPUs. Setting it to 1 disables parallelism.
  pub jobs: Option<usize>,

  /// Enables in-development features that may be changed or removed at any
  /// time.
  pub features: Vec<Feature>,

  /// The format to emit diagnostics in.
  pub diagnostic_format: DiagnosticFormat,
}

/// Arguments related to where packages are stored in the system.
#[derive(Debug, Clone)]
pub struct PackageArgs {
  /// Custom path to local packages, defaults to system-dependent location.
  pub package_path: Option<PathBuf>,

  /// Custom path to package cache, defaults to system-dependent location.
  pub package_cache_path: Option<PathBuf>,
}

/// Common arguments to customize available fonts.
#[derive(Debug, Clone)]
pub struct FontArgs {
  /// Adds additional directories that are recursively searched for fonts.
  ///
  /// If multiple paths are specified, they are separated by the system's path
  /// separator (`:` on Unix-like systems and `;` on Windows).
  pub font_paths: Vec<PathBuf>,

  /// Ensures system fonts won't be searched, unless explicitly included via
  /// `--font-path`.
  pub ignore_system_fonts: bool,
}

#[allow(dead_code)]
/// An input that is either stdin or a real path.
#[derive(Debug, Clone)]
pub enum Input {
  /// Stdin, represented by `-`.
  Stdin,
  /// A non-empty path.
  Path(PathBuf),
}

impl Display for Input {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Input::Stdin => f.pad("stdin"),
      Input::Path(path) => path.display().fmt(f),
    }
  }
}

#[allow(dead_code)]
/// An output that is either stdout or a real path.
#[derive(Debug, Clone)]
pub enum Output {
  /// Stdout, represented by `-`.
  Stdout,
  /// A non-empty path.
  Path(PathBuf),
}

impl Display for Output {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Output::Stdout => f.pad("stdout"),
      Output::Path(path) => path.display().fmt(f),
    }
  }
}

/// Which format to use for the generated output file.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum OutputFormat {
  Pdf,
  Png,
  Svg,
  Html,
}

#[allow(dead_code)]
/// Which format to use for a generated dependency file.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub enum DepsFormat {
  /// Encodes as JSON, failing for non-Unicode paths.
  #[default]
  Json,
  /// Separates paths with NULL bytes and can express all paths.
  Zero,
  /// Emits in Make format, omitting inexpressible paths.
  Make,
}

/// Which format to use for diagnostics.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiagnosticFormat {
  #[default]
  Human,
  Short,
}

#[allow(dead_code)]
/// An in-development feature that may be changed or removed at any time.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Feature {
  Html,
  A11yExtras,
}

/// A PDF standard that Typst can enforce conformance with.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum PdfStandard {
  /// PDF 1.4.
  V_1_4,
  /// PDF 1.5.
  V_1_5,
  /// PDF 1.6.
  V_1_6,
  /// PDF 1.7.
  V_1_7,
  /// PDF 2.0.
  V_2_0,
  /// PDF/A-1b.
  A_1b,
  /// PDF/A-1a.
  A_1a,
  /// PDF/A-2b.
  A_2b,
  /// PDF/A-2u.
  A_2u,
  /// PDF/A-2a.
  A_2a,
  /// PDF/A-3b.
  A_3b,
  /// PDF/A-3u.
  A_3u,
  /// PDF/A-3a.
  A_3a,
  /// PDF/A-4.
  A_4,
  /// PDF/A-4f.
  A_4f,
  /// PDF/A-4e.
  A_4e,
  /// PDF/UA-1.
  UA_1,
}

/// Implements parsing of page ranges (`1-3`, `4`, `5-`, `-2`), used by the
/// `CompileCommand.pages` argument, through the `FromStr` trait instead of a
/// value parser, in order to generate better errors.
///
/// See also: https://github.com/clap-rs/clap/issues/5065
#[derive(Debug, Clone)]
pub struct Pages(pub RangeInclusive<Option<NonZeroUsize>>);

impl FromStr for Pages {
  type Err = &'static str;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value
      .split('-')
      .map(str::trim)
      .collect::<Vec<_>>()
      .as_slice()
    {
      [] | [""] => Err("page export range must not be empty"),
      [single_page] => {
        let page_number = parse_page_number(single_page)?;
        Ok(Pages(Some(page_number)..=Some(page_number)))
      }
      ["", ""] => Err("page export range must have start or end"),
      [start, ""] => Ok(Pages(Some(parse_page_number(start)?)..=None)),
      ["", end] => Ok(Pages(None..=Some(parse_page_number(end)?))),
      [start, end] => {
        let start = parse_page_number(start)?;
        let end = parse_page_number(end)?;
        if start > end {
          Err("page export range must end at a page after the start")
        } else {
          Ok(Pages(Some(start)..=Some(end)))
        }
      }
      [_, _, _, ..] => Err("page export range must have a single hyphen"),
    }
  }
}

/// Parses a single page number.
fn parse_page_number(value: &str) -> Result<NonZeroUsize, &'static str> {
  if value == "0" {
    Err("page numbers start at one")
  } else {
    NonZeroUsize::from_str(value).map_err(|_| "not a valid page number")
  }
}
