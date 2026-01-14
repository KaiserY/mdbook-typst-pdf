mod args;
mod convert;
mod download;
mod export;
mod package;
mod terminal;
mod world;

use codespan_reporting::term::{self, termcolor};
use mdbook_renderer::RenderContext;
use mdbook_renderer::config::Config as MdConfig;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use termcolor::{ColorChoice, WriteColor};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::args::{
  CompileArgs, CompileCommand, DepsFormat, DiagnosticFormat, FontArgs, Input, Output, OutputFormat,
  PackageArgs, ProcessArgs, WorldArgs,
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
  pub pdf: bool,
  pub custom_template: Option<String>,
  pub section_number: bool,
  pub chapter_no_pagebreak: bool,
  pub rust_book: bool,
}

fn main() -> Result<(), anyhow::Error> {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "mdbook_typst_pdf=info,typst=error".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

  let mut stdin = io::stdin();

  let ctx = RenderContext::from_json(&mut stdin)?;

  let cfg: Config = ctx.config.get("output.typst-pdf")?.unwrap_or_default();

  let template_str = if let Some(custom_template) = &cfg.custom_template {
    let mut custom_template_path = ctx.root.clone();
    custom_template_path.push(custom_template);
    std::fs::read_to_string(custom_template_path)?
  } else {
    include_str!("assets/template.typ").to_string()
  };

  let typst_str = convert::convert_typst(&ctx, &cfg, &template_str)?;

  let typst_filename = output_filename(&ctx.destination, &ctx.config, "typ");

  if cfg.pdf {
    let mut tmpfile = NamedTempFile::new()?;
    tmpfile.write_all(typst_str.as_bytes())?;
    tmpfile.flush()?;

    write_file(&typst_str, &typst_filename);

    let args = CompileArgs {
      input: Input::Path(typst_filename),
      output: Some(Output::Path(output_filename(
        &ctx.destination,
        &ctx.config,
        "pdf",
      ))),
      format: Some(OutputFormat::Pdf),
      world: WorldArgs {
        inputs: vec![],
        root: None,
        font: FontArgs {
          font_paths: vec![],
          ignore_system_fonts: false,
        },
        package: PackageArgs {
          package_path: None,
          package_cache_path: None,
        },
        creation_timestamp: None,
      },
      pages: None,
      pdf_standard: vec![],
      no_pdf_tags: false,
      ppi: 0.0,
      make_deps: None,
      deps: None,
      deps_format: DepsFormat::Json,
      process: ProcessArgs {
        jobs: None,
        features: vec![],
        diagnostic_format: DiagnosticFormat::Human,
      },
      open: None,
      timings: None,
    };

    let res = crate::export::export_pdf(&CompileCommand { args });

    if let Err(msg) = res {
      print_error(&msg).expect("failed to print error");

      return Err(anyhow::anyhow!(msg));
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

fn write_file(data: &str, filename: &PathBuf) {
  let display = filename.display();

  let mut file = match File::create(filename) {
    Err(why) => panic!("Couldn't create {}: {}", display, why),
    Ok(file) => file,
  };

  if let Err(why) = file.write_all(data.as_bytes()) {
    panic!("Couldn't write to {}: {}", display, why)
  }
}

fn output_filename(dest: &Path, config: &MdConfig, extension: &str) -> PathBuf {
  match config.book.title {
    Some(ref title) => dest.join(title).with_extension(extension),
    None => dest.join("book").with_extension(extension),
  }
}
