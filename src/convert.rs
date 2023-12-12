use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use std::fmt::Write;

#[derive(Debug, Default)]
struct ConvertContext {}

#[derive(Debug)]
pub enum EventType {
  CodeBlock,
  List,
  NumberedList,
}

pub fn convert_typst(ctx: &RenderContext, template: &str) -> Result<String, anyhow::Error> {
  let mut output_template = template.to_owned();

  let mut typst_str = String::new();

  let convert_context = ConvertContext::default();

  for item in ctx.book.iter() {
    writeln!(typst_str, "{}", convert_book_item(&convert_context, item)?)?;
  }

  let placeholder = "/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/\n";
  let target = output_template.find(placeholder).unwrap_or_default() + placeholder.len();

  output_template.insert_str(target, &typst_str);

  Ok(output_template)
}

fn convert_book_item(ctx: &ConvertContext, item: &BookItem) -> Result<String, anyhow::Error> {
  let mut book_item_str = String::new();

  if let BookItem::Chapter(ref ch) = *item {
    let number = match &ch.number {
      Some(number) => {
        writeln!(
          book_item_str,
          "{} {} {}",
          "=".repeat(number.len()),
          number,
          ch.name
        )?;
        number.len()
      }
      None => {
        writeln!(book_item_str, "= {}", ch.name)?;
        1
      }
    };

    writeln!(
      book_item_str,
      "{}#pagebreak(weak: true)",
      convert_content(ctx, &ch.content, number)?
    )?;
  }

  Ok(book_item_str)
}

fn convert_content(
  _ctx: &ConvertContext,
  content: &str,
  number: usize,
) -> Result<String, anyhow::Error> {
  let mut content_str = String::new();

  let options = Options::ENABLE_SMART_PUNCTUATION
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_FOOTNOTES
    | Options::ENABLE_TASKLISTS
    | Options::ENABLE_TABLES;

  let parser = Parser::new_ext(content, options);

  let mut event_stack = Vec::new();

  for event in parser {
    match event {
      Event::Start(Tag::Heading(level, _, _)) => match level {
        HeadingLevel::H1 => write!(content_str, "/*")?,
        HeadingLevel::H2 => {
          if number > 1 {
            write!(content_str, "/*")?
          } else {
            write!(content_str, "#heading(level:2, outlined: false)[")?
          }
        }
        HeadingLevel::H3 => write!(content_str, "#heading(level:3, outlined: false)[")?,
        HeadingLevel::H4 => write!(content_str, "#heading(level:4, outlined: false)[")?,
        HeadingLevel::H5 => write!(content_str, "#heading(level:5, outlined: false)[")?,
        HeadingLevel::H6 => write!(content_str, "#heading(level:6, outlined: false)[")?,
      },
      Event::End(Tag::Heading(level, _, _)) => match level {
        HeadingLevel::H1 => writeln!(content_str, "*/")?,
        HeadingLevel::H2 => {
          if number > 1 {
            writeln!(content_str, "*/")?
          } else {
            writeln!(content_str, "]")?
          }
        }
        _ => writeln!(content_str, "]")?,
      },
      Event::Start(Tag::Emphasis) => write!(content_str, "_")?,
      Event::End(Tag::Emphasis) => write!(content_str, "_")?,
      Event::Start(Tag::Strong) => write!(content_str, "*")?,
      Event::End(Tag::Strong) => write!(content_str, "*")?,
      Event::Start(Tag::BlockQuote) => write!(content_str, "#quote(block: true)[")?,
      Event::End(Tag::BlockQuote) => writeln!(content_str, "]")?,
      Event::Start(Tag::List(None)) => {
        event_stack.push(EventType::List);
      }
      Event::End(Tag::List(None)) => {
        event_stack.pop();
      }
      Event::Start(Tag::List(Some(_))) => {
        event_stack.push(EventType::NumberedList);
      }
      Event::End(Tag::List(Some(_))) => {
        event_stack.pop();
      }
      Event::Start(Tag::Item) => match event_stack.last() {
        Some(EventType::List) => write!(content_str, "- ")?,
        Some(EventType::NumberedList) => write!(content_str, "+ ")?,
        _ => write!(content_str, "- ")?,
      },
      Event::End(Tag::Item) => writeln!(content_str)?,
      Event::Start(Tag::Paragraph) => (),
      Event::End(Tag::Paragraph) => write!(content_str, "\n\n")?,
      Event::Start(Tag::Link(_, url, _)) => write!(content_str, "#link(\"{}\")[", url)?,
      Event::End(Tag::Link(_, _, _)) => write!(content_str, "]")?,
      Event::Start(Tag::CodeBlock(ref lang)) => {
        event_stack.push(EventType::CodeBlock);
        match lang {
          CodeBlockKind::Indented => writeln!(content_str, "```")?,
          CodeBlockKind::Fenced(lang) => writeln!(content_str, "```{}", lang)?,
        }
      }
      Event::End(Tag::CodeBlock(_)) => {
        event_stack.pop();
        writeln!(content_str, "```")?
      }
      Event::Code(t) => write!(content_str, "`{}`", t)?,
      Event::Text(t) => match event_stack.last() {
        Some(EventType::CodeBlock) => write!(content_str, "{}", t)?,
        _ => write!(content_str, "{}", t.replace('#', "\\#").replace('$', "\\$"))?,
      },
      Event::SoftBreak => writeln!(content_str, " \\")?,
      _ => (),
    }
  }

  Ok(content_str)
}
