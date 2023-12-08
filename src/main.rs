mod convert;
mod download;
mod export;
mod fonts;
mod package;
mod world;

use codespan_reporting::term::{self, termcolor};
use export::ExportArgs;
use mdbook::config::Config as MdConfig;
use mdbook::renderer::RenderContext;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use termcolor::{ColorChoice, WriteColor};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
  pub typst: bool,
  pub pdf: bool,
  pub custom_template: Option<String>,
}

fn main() -> Result<(), anyhow::Error> {
  tracing_subscriber::fmt()
    .without_time()
    .with_max_level(tracing::Level::INFO)
    .init();

  let mut stdin = io::stdin();

  let ctx = RenderContext::from_json(&mut stdin)?;

  let cfg: Config = ctx
    .config
    .get_deserialized_opt("output.typst-pdf")?
    .unwrap_or_default();

  let template_str = if let Some(custom_template) = &cfg.custom_template {
    let mut custom_template_path = ctx.root.clone();
    custom_template_path.push(custom_template);
    std::fs::read_to_string(custom_template_path)?
  } else {
    include_str!("assets/template.typ").to_string()
  };

  let typst_str = convert::convert_typst(&ctx, &template_str)?;

  if cfg.typst {
    let filename = output_filename(&ctx.destination, &ctx.config, "typ");
    write_file(&typst_str, filename);
  }

  if cfg.pdf {
    let mut tmpfile = NamedTempFile::new()?;
    tmpfile.write_all(typst_str.as_bytes())?;
    tmpfile.flush()?;

    let args = ExportArgs {
      input: PathBuf::from(tmpfile.path()),
      output: output_filename(&ctx.destination, &ctx.config, "pdf"),
      root: None,
      font_paths: vec![],
    };

    let res = crate::export::export_pdf(args);

    if let Err(msg) = res {
      print_error(&msg).expect("failed to print error");
    }
  }

  Ok(())
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

fn write_file(data: &str, filename: PathBuf) {
  let display = filename.display();

  let mut file = match File::create(&filename) {
    Err(why) => panic!("Couldn't create {}: {}", display, why.to_string()),
    Ok(file) => file,
  };

  if let Err(why) = file.write_all(data.as_bytes()) {
    panic!("Couldn't write to {}: {}", display, why.to_string())
  }
}

fn output_filename(dest: &Path, config: &MdConfig, extension: &str) -> PathBuf {
  match config.book.title {
    Some(ref title) => dest.join(title).with_extension(extension),
    None => dest.join("book").with_extension(extension),
  }
}
