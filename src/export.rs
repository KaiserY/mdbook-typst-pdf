use chrono::{Datelike, Timelike};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use ecow::eco_format;
use std::fs;
use typst::diag::Warned;
use typst::diag::{At, Severity, SourceDiagnostic, StrResult};
use typst::foundations::Datetime;
use typst::foundations::Smart;
use typst::syntax::{FileId, Source, Span};
use typst::{World, WorldExt};
use typst_pdf::{PdfOptions, PdfStandards};

use crate::args::{DiagnosticFormat, SharedArgs};
use crate::terminal;
use crate::world::SystemWorld;

type CodespanResult<T> = Result<T, CodespanError>;
type CodespanError = codespan_reporting::files::Error;

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

  let Warned { output, warnings } = typst::compile(&world);

  let result = output.and_then(|document| {
    let options = PdfOptions {
      ident: Smart::Auto,
      timestamp: convert_datetime(chrono::Utc::now()),
      page_ranges: None,
      standards: pdf_standards().at(Span::detached())?,
    };

    let buffer = typst_pdf::pdf(&document, &options)?;

    fs::write(args.output, buffer)
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

/// The PDF standards to try to conform with.
fn pdf_standards() -> StrResult<PdfStandards> {
  let list = vec![];

  PdfStandards::new(&list)
}

/// Convert [`chrono::DateTime`] to [`Datetime`]
fn convert_datetime(date_time: chrono::DateTime<chrono::Utc>) -> Option<Datetime> {
  Datetime::from_ymd_hms(
    date_time.year(),
    date_time.month().try_into().ok()?,
    date_time.day().try_into().ok()?,
    date_time.hour().try_into().ok()?,
    date_time.minute().try_into().ok()?,
    date_time.second().try_into().ok()?,
  )
}
