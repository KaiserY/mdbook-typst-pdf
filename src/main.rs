mod args;
mod compile;
mod download;
mod fonts;
mod package;
mod tracing;
mod watch;
mod world;

use clap::Parser;
use codespan_reporting::term::{self, termcolor};
use once_cell::sync::Lazy;
use std::cell::Cell;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use termcolor::{ColorChoice, WriteColor};

use crate::args::{CliArguments, Command};

thread_local! {
  /// The CLI's exit code.
  static EXIT: Cell<ExitCode> = Cell::new(ExitCode::SUCCESS);
}

static ARGS: Lazy<CliArguments> = Lazy::new(CliArguments::parse);

fn main() -> ExitCode {
  let _guard = match crate::tracing::setup_tracing(&ARGS) {
    Ok(guard) => guard,
    Err(err) => {
      eprintln!("failed to initialize tracing ({err})");
      None
    }
  };

  let res = match &ARGS.command {
    Command::Compile(command) => crate::compile::compile(command.clone()),
    _ => Ok(()),
  };

  if let Err(msg) = res {
    set_failed();
    print_error(&msg).expect("failed to print error");
  }

  EXIT.with(|cell| cell.get())
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
