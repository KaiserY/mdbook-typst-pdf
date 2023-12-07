mod download;
mod export;
mod fonts;
mod package;
mod world;

use codespan_reporting::term::{self, termcolor};
use export::ExportArgs;
use mdbook::book::Chapter;
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use once_cell::sync::Lazy;
use std::cell::Cell;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use termcolor::{ColorChoice, WriteColor};

pub struct CliArguments {
  pub cert: Option<PathBuf>,
}

thread_local! {
  /// The CLI's exit code.
  static EXIT: Cell<ExitCode> = Cell::new(ExitCode::SUCCESS);
}

static ARGS: Lazy<CliArguments> = Lazy::new(|| CliArguments { cert: None });

fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .without_time()
    .with_max_level(tracing::Level::INFO)
    .init();

  let mut stdin = io::stdin();

  let ctx = RenderContext::from_json(&mut stdin)?;

  for item in ctx.book.iter() {
    if let BookItem::Chapter(ref ch) = *item {
      let num_words = count_words(ch);
      tracing::info!("{}: {}", ch.name, num_words);
    }
  }

  let args = ExportArgs {
    input: PathBuf::from("test.typ"),
    output: PathBuf::from("test.pdf"),
    root: None,
    font_paths: vec![],
  };

  let res = crate::export::export_pdf(args);

  if let Err(msg) = res {
    set_failed();
    print_error(&msg).expect("failed to print error");
  }

  Ok(())
}

fn count_words(ch: &Chapter) -> usize {
  ch.content.split_whitespace().count()
}

fn color_stream() -> termcolor::StandardStream {
  termcolor::StandardStream::stderr(if std::io::stderr().is_terminal() {
    ColorChoice::Auto
  } else {
    ColorChoice::Never
  })
}

fn set_failed() {
  EXIT.with(|cell| cell.set(ExitCode::FAILURE));
}

fn print_error(msg: &str) -> io::Result<()> {
  let mut w = color_stream();
  let styles = term::Styles::default();

  w.set_color(&styles.header_error)?;
  write!(w, "error")?;

  w.reset()?;
  writeln!(w, ": {msg}.")
}
