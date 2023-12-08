use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::{Event, Tag};
use std::fmt::Write;

#[derive(Debug, Default)]
struct ConvertContext {}

pub fn convert_typst(ctx: &RenderContext, template: &String) -> Result<String, anyhow::Error> {
  let mut output_template = template.clone();

  let mut typst_str = String::new();

  let convert_context = ConvertContext::default();

  for item in ctx.book.iter() {
    writeln!(typst_str, "{}", convert_book_item(&convert_context, item)?)?;
  }

  let placeholder = "/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/\n";
  let target = output_template.find(&placeholder).unwrap_or_default() + placeholder.len();

  output_template.insert_str(target, &typst_str);

  Ok(output_template)
}

fn convert_book_item(ctx: &ConvertContext, item: &BookItem) -> Result<String, anyhow::Error> {
  let mut book_item_str = String::new();

  if let BookItem::Chapter(ref ch) = *item {
    match &ch.number {
      Some(number) => writeln!(
        book_item_str,
        "{} {} {}",
        "=".repeat(number.len()),
        number,
        ch.name
      )?,
      None => writeln!(book_item_str, "= {}", ch.name)?,
    }

    writeln!(book_item_str, "{}", convert_content(ctx, &ch.content)?)?;
  }

  Ok(book_item_str)
}

fn convert_content(ctx: &ConvertContext, content: &str) -> Result<String, anyhow::Error> {
  let mut content_str = String::new();

  let options = Options::ENABLE_SMART_PUNCTUATION
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_FOOTNOTES
    | Options::ENABLE_TASKLISTS
    | Options::ENABLE_TABLES;

  let parser = Parser::new_ext(content, options);

  for event in parser {
    match event {
      Event::Start(Tag::Heading(level, _, _)) => {}
      _ => (),
    }
  }

  Ok(content_str)
}
