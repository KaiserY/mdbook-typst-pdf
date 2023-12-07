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
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use termcolor::{ColorChoice, WriteColor};

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

fn print_error(msg: &str) -> io::Result<()> {
  let mut w = color_stream();
  let styles = term::Styles::default();

  w.set_color(&styles.header_error)?;
  write!(w, "error")?;

  w.reset()?;
  writeln!(w, ": {msg}.")
}
