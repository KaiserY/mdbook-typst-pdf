use chrono::{Datelike, Timelike};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use ecow::eco_format;
use std::fs;
use std::path::PathBuf;

use typst::diag::{At, Severity, SourceDiagnostic, StrResult};
use typst::eval::Tracer;
use typst::foundations::Datetime;
use typst::foundations::Smart;
use typst::syntax::{FileId, Source, Span};
use typst::{World, WorldExt};

use crate::terminal;
use crate::world::SystemWorld;

type CodespanResult<T> = Result<T, CodespanError>;
type CodespanError = codespan_reporting::files::Error;

#[derive(Debug, Clone)]
pub struct ExportArgs {
  pub input: PathBuf,
  pub root: Option<PathBuf>,
  pub font_paths: Vec<PathBuf>,
  pub output: PathBuf,
}

/// Common arguments of compile, watch, and query.
#[derive(Debug, Clone)]
pub struct SharedArgs {
  pub input: Input,
  pub root: Option<PathBuf>,
  pub inputs: Vec<(String, String)>,
  pub font_paths: Vec<PathBuf>,
  pub output: PathBuf,
}

/// Which format to use for diagnostics.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiagnosticFormat {
  Human,
  Short,
}

pub fn export_pdf(args: SharedArgs) -> StrResult<()> {
  let world = SystemWorld::new(&args).map_err(|err| eco_format!("{err}"))?;

  tracing::info!("Starting compilation");

  let start = std::time::Instant::now();

  // Check if main file can be read and opened.
  if let Err(errors) = world.source(world.main()).at(Span::detached()) {
    print_diagnostics(&world, &errors, &[], DiagnosticFormat::Human)
      .map_err(|err| eco_format!("failed to print diagnostics ({err})"))?;

    return Err(eco_format!("export_pdf failed"));
  }

  let mut tracer = Tracer::new();
  let result = typst::compile(&world, &mut tracer);

  match result {
    Ok(document) => {
      let buffer = typst_pdf::pdf(&document, Smart::Auto, now());

      fs::write(args.output, buffer)
        .map_err(|err| eco_format!("failed to write PDF file ({err})"))?;

      let duration = start.elapsed();

      tracing::info!("Compilation succeeded in {duration:?}");
    }
    Err(errors) => {
      print_diagnostics(&world, &errors, &[], DiagnosticFormat::Human)
        .map_err(|err| eco_format!("failed to print diagnostics ({err})"))?;
    }
  }

  Ok(())
}

/// Get the current date and time in UTC.
fn now() -> Option<Datetime> {
  let now = chrono::Local::now().naive_utc();
  Datetime::from_ymd_hms(
    now.year(),
    now.month().try_into().ok()?,
    now.day().try_into().ok()?,
    now.hour().try_into().ok()?,
    now.minute().try_into().ok()?,
    now.second().try_into().ok()?,
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

    term::emit(&mut terminal::out(), &config, world, &diag)?;

    // Stacktrace-like helper diagnostics.
    for point in &diagnostic.trace {
      let message = point.v.to_string();
      let help = Diagnostic::help()
        .with_message(message)
        .with_labels(label(world, point.span).into_iter().collect());

      term::emit(&mut terminal::out(), &config, world, &help)?;
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
  type Source = Source;

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

/// An input that is either stdin or a real path.
#[derive(Debug, Clone)]
pub enum Input {
  /// Stdin, represented by `-`.
  #[allow(dead_code)]
  Stdin,
  /// A non-empty path.
  Path(PathBuf),
}
