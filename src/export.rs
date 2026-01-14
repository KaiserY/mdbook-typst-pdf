use chrono::{DateTime, Datelike, Timelike, Utc};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use ecow::eco_format;
use parking_lot::RwLock;
use std::io::Write;
use std::path::PathBuf;
use typst::WorldExt;
use typst::diag::{At, Severity, SourceDiagnostic, StrResult, Warned, bail};
use typst::foundations::Datetime;
use typst::foundations::Smart;
use typst::layout::{Frame, PageRanges, PagedDocument};
use typst::syntax::{FileId, Lines, Span};
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

use crate::args::{
  CompileArgs, CompileCommand, DiagnosticFormat, Input, Output, OutputFormat, PdfStandard,
  WatchCommand,
};
use crate::terminal;
use crate::world::SystemWorld;

type CodespanResult<T> = Result<T, CodespanError>;
type CodespanError = codespan_reporting::files::Error;

pub fn export_pdf(command: &CompileCommand) -> StrResult<()> {
  let config = CompileConfig::new(command)?;

  let world = SystemWorld::new(
    &command.args.input,
    &command.args.world,
    &command.args.process,
  )
  .map_err(|err| eco_format!("{err}"))?;

  tracing::info!("Starting compilation");

  let start = std::time::Instant::now();

  let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);

  let result = output.and_then(|document| {
    let timestamp = match config.creation_timestamp {
      Some(timestamp) => convert_datetime(timestamp).map(Timestamp::new_utc),
      None => {
        let local_datetime = chrono::Local::now();
        convert_datetime(local_datetime).and_then(|datetime| {
          Timestamp::new_local(datetime, local_datetime.offset().local_minus_utc() / 60)
        })
      }
    };

    let options = PdfOptions {
      ident: Smart::Auto,
      timestamp,
      page_ranges: config.pages.clone(),
      standards: config.pdf_standards.clone(),
      tagged: true,
    };

    let buffer = typst_pdf::pdf(&document, &options)?;

    config
      .output
      .write(&buffer)
      .map_err(|err| eco_format!("failed to write PDF file ({err})"))
      .at(Span::detached())?;

    Ok(())
  });

  match result {
    Ok(()) => {
      let duration = start.elapsed();

      tracing::info!("Compilation succeeded in {duration:?}");

      print_diagnostics(&world, &[], &warnings, DiagnosticFormat::Human)
        .map_err(|err| eco_format!("failed to print diagnostics ({err})"))?;
    }
    Err(errors) => {
      print_diagnostics(&world, &errors, &[], DiagnosticFormat::Human)
        .map_err(|err| eco_format!("failed to print diagnostics ({err})"))?;

      return Err(eco_format!("export_pdf failed"));
    }
  }

  Ok(())
}

#[allow(dead_code)]
/// A preprocessed `CompileCommand`.
pub struct CompileConfig {
  /// Whether we are watching.
  pub watching: bool,
  /// Path to input Typst file or stdin.
  pub input: Input,
  /// Path to output file (PDF, PNG, SVG, or HTML).
  pub output: Output,
  /// The format of the output file.
  pub output_format: OutputFormat,
  /// Which pages to export.
  pub pages: Option<PageRanges>,
  /// The document's creation date formatted as a UNIX timestamp, with UTC suffix.
  pub creation_timestamp: Option<DateTime<Utc>>,
  /// The format to emit diagnostics in.
  pub diagnostic_format: DiagnosticFormat,
  /// Opens the output file with the default viewer or a specific program after
  /// compilation.
  pub open: Option<Option<String>>,
  /// One (or multiple comma-separated) PDF standards that Typst will enforce
  /// conformance with.
  pub pdf_standards: PdfStandards,
  /// A path to write a Makefile rule describing the current compilation.
  pub make_deps: Option<PathBuf>,
  /// The PPI (pixels per inch) to use for PNG export.
  pub ppi: f32,
  /// The export cache for images, used for caching output files in `typst
  /// watch` sessions with images.
  pub export_cache: ExportCache,
}

impl CompileConfig {
  /// Preprocess a `CompileCommand`, producing a compilation config.
  pub fn new(command: &CompileCommand) -> StrResult<Self> {
    Self::new_impl(&command.args, None)
  }

  /// The shared implementation of [`CompileConfig::new`] and
  /// [`CompileConfig::watching`].
  fn new_impl(args: &CompileArgs, watch: Option<&WatchCommand>) -> StrResult<Self> {
    let input = args.input.clone();

    let output_format = if let Some(specified) = args.format {
      specified
    } else if let Some(Output::Path(output)) = &args.output {
      match output.extension() {
        Some(ext) if ext.eq_ignore_ascii_case("pdf") => OutputFormat::Pdf,
        Some(ext) if ext.eq_ignore_ascii_case("png") => OutputFormat::Png,
        Some(ext) if ext.eq_ignore_ascii_case("svg") => OutputFormat::Svg,
        Some(ext) if ext.eq_ignore_ascii_case("html") => OutputFormat::Html,
        _ => bail!(
          "could not infer output format for path {}.\n\
                   consider providing the format manually with `--format/-f`",
          output.display()
        ),
      }
    } else {
      OutputFormat::Pdf
    };

    let output = args.output.clone().unwrap_or_else(|| {
      let Input::Path(path) = &input else {
        panic!("output must be specified when input is from stdin, as guarded by the CLI");
      };
      Output::Path(path.with_extension(match output_format {
        OutputFormat::Pdf => "pdf",
        OutputFormat::Png => "png",
        OutputFormat::Svg => "svg",
        OutputFormat::Html => "html",
      }))
    });

    let pages = args
      .pages
      .as_ref()
      .map(|export_ranges| PageRanges::new(export_ranges.iter().map(|r| r.0.clone()).collect()));

    let pdf_standards = PdfStandards::new(
      &args
        .pdf_standard
        .iter()
        .copied()
        .map(Into::into)
        .collect::<Vec<_>>(),
    )?;

    Ok(Self {
      watching: watch.is_some(),
      input,
      output,
      output_format,
      pages,
      pdf_standards,
      creation_timestamp: args.world.creation_timestamp,
      make_deps: args.make_deps.clone(),
      ppi: args.ppi,
      diagnostic_format: args.process.diagnostic_format,
      open: args.open.clone(),
      export_cache: ExportCache::new(),
    })
  }
}

#[allow(dead_code)]
/// Caches exported files so that we can avoid re-exporting them if they haven't
/// changed.
///
/// This is done by having a list of size `files.len()` that contains the hashes
/// of the last rendered frame in each file. If a new frame is inserted, this
/// will invalidate the rest of the cache, this is deliberate as to decrease the
/// complexity and memory usage of such a cache.
pub struct ExportCache {
  /// The hashes of last compilation's frames.
  pub cache: RwLock<Vec<u128>>,
}

#[allow(dead_code)]
impl ExportCache {
  /// Creates a new export cache.
  pub fn new() -> Self {
    Self {
      cache: RwLock::new(Vec::with_capacity(32)),
    }
  }

  /// Returns true if the entry is cached and appends the new hash to the
  /// cache (for the next compilation).
  pub fn is_cached(&self, i: usize, frame: &Frame) -> bool {
    let hash = typst::utils::hash128(frame);

    let mut cache = self.cache.upgradable_read();
    if i >= cache.len() {
      cache.with_upgraded(|cache| cache.push(hash));
      return false;
    }

    cache.with_upgraded(|cache| std::mem::replace(&mut cache[i], hash) == hash)
  }
}

/// Convert [`chrono::DateTime`] to [`Datetime`]
fn convert_datetime<Tz: chrono::TimeZone>(date_time: chrono::DateTime<Tz>) -> Option<Datetime> {
  Datetime::from_ymd_hms(
    date_time.year(),
    date_time.month().try_into().ok()?,
    date_time.day().try_into().ok()?,
    date_time.hour().try_into().ok()?,
    date_time.minute().try_into().ok()?,
    date_time.second().try_into().ok()?,
  )
}

/// Print diagnostic messages to the terminal.
pub fn print_diagnostics(
  world: &SystemWorld,
  errors: &[SourceDiagnostic],
  warnings: &[SourceDiagnostic],
  diagnostic_format: DiagnosticFormat,
) -> Result<(), codespan_reporting::files::Error> {
  let mut config = term::Config {
    tab_width: 2,
    ..Default::default()
  };
  if diagnostic_format == DiagnosticFormat::Short {
    config.display_style = term::DisplayStyle::Short;
  }

  for diagnostic in warnings.iter().chain(errors) {
    let diag = match diagnostic.severity {
      Severity::Error => Diagnostic::error(),
      Severity::Warning => Diagnostic::warning(),
    }
    .with_message(diagnostic.message.clone())
    .with_notes(
      diagnostic
        .hints
        .iter()
        .map(|e| (eco_format!("hint: {e}")).into())
        .collect(),
    )
    .with_labels(label(world, diagnostic.span).into_iter().collect());

    term::emit_to_io_write(&mut terminal::out(), &config, world, &diag)?;

    // Stacktrace-like helper diagnostics.
    for point in &diagnostic.trace {
      let message = point.v.to_string();
      let help = Diagnostic::help()
        .with_message(message)
        .with_labels(label(world, point.span).into_iter().collect());

      term::emit_to_io_write(&mut terminal::out(), &config, world, &help)?;
    }
  }

  Ok(())
}

/// Create a label for a span.
fn label(world: &SystemWorld, span: Span) -> Option<Label<FileId>> {
  Some(Label::primary(span.id()?, world.range(span)?))
}

impl<'a> codespan_reporting::files::Files<'a> for SystemWorld {
  type FileId = FileId;
  type Name = String;
  type Source = Lines<String>;

  fn name(&'a self, id: FileId) -> CodespanResult<Self::Name> {
    let vpath = id.vpath();
    Ok(if let Some(package) = id.package() {
      format!("{package}{}", vpath.as_rooted_path().display())
    } else {
      // Try to express the path relative to the working directory.
      vpath
        .resolve(self.root())
        .and_then(|abs| pathdiff::diff_paths(abs, self.workdir()))
        .as_deref()
        .unwrap_or_else(|| vpath.as_rootless_path())
        .to_string_lossy()
        .into()
    })
  }

  fn source(&'a self, id: FileId) -> CodespanResult<Self::Source> {
    Ok(self.lookup(id))
  }

  fn line_index(&'a self, id: FileId, given: usize) -> CodespanResult<usize> {
    let source = self.lookup(id);
    source
      .byte_to_line(given)
      .ok_or_else(|| CodespanError::IndexTooLarge {
        given,
        max: source.len_bytes(),
      })
  }

  fn line_range(&'a self, id: FileId, given: usize) -> CodespanResult<std::ops::Range<usize>> {
    let source = self.lookup(id);
    source
      .line_to_range(given)
      .ok_or_else(|| CodespanError::LineTooLarge {
        given,
        max: source.len_lines(),
      })
  }

  fn column_number(&'a self, id: FileId, _: usize, given: usize) -> CodespanResult<usize> {
    let source = self.lookup(id);
    source.byte_to_column(given).ok_or_else(|| {
      let max = source.len_bytes();
      if given <= max {
        CodespanError::InvalidCharBoundary { given }
      } else {
        CodespanError::IndexTooLarge { given, max }
      }
    })
  }
}

impl Output {
  /// Write data to the output.
  pub fn write(&self, buffer: &[u8]) -> std::io::Result<()> {
    match self {
      Output::Stdout => std::io::stdout().write_all(buffer),
      Output::Path(path) => std::fs::write(path, buffer),
    }
  }
}

impl From<PdfStandard> for typst_pdf::PdfStandard {
  fn from(standard: PdfStandard) -> Self {
    match standard {
      PdfStandard::V_1_4 => typst_pdf::PdfStandard::V_1_4,
      PdfStandard::V_1_5 => typst_pdf::PdfStandard::V_1_5,
      PdfStandard::V_1_6 => typst_pdf::PdfStandard::V_1_6,
      PdfStandard::V_1_7 => typst_pdf::PdfStandard::V_1_7,
      PdfStandard::V_2_0 => typst_pdf::PdfStandard::V_2_0,
      PdfStandard::A_1b => typst_pdf::PdfStandard::A_1b,
      PdfStandard::A_1a => typst_pdf::PdfStandard::A_1a,
      PdfStandard::A_2b => typst_pdf::PdfStandard::A_2b,
      PdfStandard::A_2u => typst_pdf::PdfStandard::A_2u,
      PdfStandard::A_2a => typst_pdf::PdfStandard::A_2a,
      PdfStandard::A_3b => typst_pdf::PdfStandard::A_3b,
      PdfStandard::A_3u => typst_pdf::PdfStandard::A_3u,
      PdfStandard::A_3a => typst_pdf::PdfStandard::A_3a,
      PdfStandard::A_4 => typst_pdf::PdfStandard::A_4,
      PdfStandard::A_4f => typst_pdf::PdfStandard::A_4f,
      PdfStandard::A_4e => typst_pdf::PdfStandard::A_4e,
      PdfStandard::UA_1 => typst_pdf::PdfStandard::Ua_1,
    }
  }
}
