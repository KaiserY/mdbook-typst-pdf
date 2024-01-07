use anyhow::anyhow;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use mdbook::book::SectionNumber;
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag};
use std::fmt::Write;
use std::fs;

use crate::Config;

#[derive(Debug)]
pub enum EventType {
  CodeBlock,
  List,
  NumberedList,
  TableHead,
  Image,
}

pub fn convert_typst(
  ctx: &RenderContext,
  cfg: &Config,
  template: &str,
) -> Result<String, anyhow::Error> {
  let title = ctx
    .config
    .book
    .title
    .as_ref()
    .ok_or(anyhow!("title not found"))?;

  let mut output_template = template.to_owned().replace("MDBOOK_TYPST_PDF_TITLE", title);

  let mut typst_str = String::new();

  for item in ctx.book.iter() {
    writeln!(typst_str, "{}", convert_book_item(ctx, cfg, item)?)?;
  }

  let placeholder = "/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/\n";
  let target = output_template.find(placeholder).unwrap_or_default() + placeholder.len();

  output_template.insert_str(target, &typst_str);

  Ok(output_template)
}

fn convert_book_item(
  ctx: &RenderContext,
  cfg: &Config,
  item: &BookItem,
) -> Result<String, anyhow::Error> {
  let mut book_item_str = String::new();

  if let BookItem::Chapter(ref ch) = *item {
    let label_path = ch
      .source_path
      .to_owned()
      .ok_or(anyhow!("source_path not found"))?;

    let label = label_path
      .as_path()
      .file_name()
      .and_then(|f| f.to_str())
      .and_then(|f| f.split('.').next())
      .ok_or(anyhow!("source_path not found"))?;

    if let Some(number) = &ch.number {
      if cfg.section_number {
        writeln!(
          book_item_str,
          "{} {} {} <{}.html>",
          "=".repeat(number.len()),
          number,
          ch.name,
          label,
        )?;
      } else {
        writeln!(
          book_item_str,
          "#{{\n  show heading: none\n  heading(level: {}, outlined: true)[{}]\n}} <{}.html>",
          number.len(),
          ch.name,
          label,
        )?;
      }
    } else {
      writeln!(
        book_item_str,
        "#{{\n  show heading: none\n  heading(level: 1, outlined: true)[{}]\n}} <{}.html>",
        ch.name, label
      )?;
    }

    writeln!(
      book_item_str,
      "{}#pagebreak(weak: true)",
      convert_content(ctx, cfg, &ch.content, &ch.number)?
    )?;
  }

  Ok(book_item_str)
}

fn convert_content(
  ctx: &RenderContext,
  cfg: &Config,
  content: &str,
  number: &Option<SectionNumber>,
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
      Event::Start(Tag::Heading(level, _, _)) => {
        let level_usize: usize = level as usize;

        if cfg.section_number && number.clone().map(|n| n.len()).unwrap_or(1) >= level_usize {
          write!(
            content_str,
            "#{{\n  show heading: none\n  heading(level: {}, outlined: false)[",
            level_usize,
          )?;
        } else {
          write!(
            content_str,
            "#heading(level: {}, outlined: false)[",
            level_usize,
          )?;
        }
      }
      Event::End(Tag::Heading(level, _, _)) => {
        let level_usize: usize = level as usize;

        if cfg.section_number && number.clone().map(|n| n.len()).unwrap_or(1) >= level_usize {
          writeln!(content_str, "]\n}}")?
        } else {
          writeln!(content_str, "]")?
        }
      }
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
      Event::Start(Tag::Link(_, url, _)) => {
        if url.starts_with("http") {
          write!(content_str, "#link(\"{}\")[", url)?
        } else {
          let url_label = url
            .split('#')
            .next()
            .ok_or(anyhow!("url_label not found"))?;

          if url_label.ends_with("html") {
            write!(content_str, "#link(<{}>)[", url_label)?
          } else {
            write!(content_str, "#link(\"{}\")[", url)?
          }
        }
      }
      Event::End(Tag::Link(_, _, _)) => write!(content_str, "]")?,
      Event::Start(Tag::Table(align)) => {
        let typst_align = align
          .iter()
          .map(|a| match a {
            Alignment::None => "auto",
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
          })
          .collect::<Vec<&str>>()
          .join(", ");

        writeln!(
          content_str,
          "#table(\n  columns: {},\n  inset: 10pt,\n  align: ({}),\n  ",
          align.len(),
          typst_align
        )?
      }
      Event::End(Tag::Table(_)) => writeln!(content_str, ")")?,
      Event::Start(Tag::TableHead) => {
        event_stack.push(EventType::TableHead);
      }
      Event::End(Tag::TableHead) => {
        event_stack.pop();
      }
      Event::Start(Tag::TableRow) => (),
      Event::End(Tag::TableRow) => (),
      Event::Start(Tag::TableCell) => write!(content_str, "[")?,
      Event::End(Tag::TableCell) => writeln!(content_str, "],")?,
      Event::Start(Tag::Image(_, path, _title)) => {
        event_stack.push(EventType::Image);

        let src_path = ctx
          .root
          .join(
            ctx
              .config
              .book
              .src
              .to_str()
              .ok_or(anyhow!("src not found"))?,
          )
          .join(path.to_string());
        let dest_path = ctx.destination.join(path.to_string());

        let dest_dir = dest_path.parent().ok_or(anyhow!("destination not found"))?;

        fs::create_dir_all(dest_dir)?;

        fs::copy(src_path, dest_path)?;

        write!(content_str, "#figure(\n  image(\"{}\")\n)", path)?
      }
      Event::End(Tag::Image(_, _path, _title)) => {
        event_stack.pop();

        writeln!(content_str)?
      }
      Event::Start(Tag::CodeBlock(ref lang)) => {
        event_stack.push(EventType::CodeBlock);
        match lang {
          CodeBlockKind::Indented => writeln!(content_str, "````")?,
          CodeBlockKind::Fenced(lang) => {
            let langs: Vec<&str> = lang.split(',').collect();

            if !langs.is_empty() {
              writeln!(content_str, "````{}", langs[0])?
            } else {
              writeln!(content_str, "````")?
            }
          }
        }
      }
      Event::End(Tag::CodeBlock(_)) => {
        event_stack.pop();
        writeln!(content_str, "````")?
      }
      Event::Code(t) => write!(content_str, "```` {} ````", t)?,
      Event::Html(t) => {
        match t.to_string().as_str() {
          "<sup>" => {
            write!(content_str, "#super[")?;
            continue;
          }
          "</sup>" => {
            write!(content_str, "]")?;
            continue;
          }
          _ => (),
        }

        let dom = parse_document(RcDom::default(), Default::default())
          .from_utf8()
          .read_from(&mut t.as_bytes())?;

        let dom_children = &dom.document.children.borrow();

        if dom_children.len() > 0 && matches!(dom_children[0].data, NodeData::Element { .. }) {
          let html_children = &dom_children[0].children.borrow();

          if html_children.len() > 1 {
            let body_children = &html_children[1].children.borrow();

            if body_children.len() > 0 {
              if let NodeData::Element { name, attrs, .. } = &body_children[0].data {
                match name.local.as_ref() {
                  "img" => {
                    for attr in attrs.borrow().iter() {
                      if attr.name.local.as_ref() == "src" {
                        let attr_src_path = attr.value.to_string();

                        let src_path = ctx
                          .root
                          .join(
                            ctx
                              .config
                              .book
                              .src
                              .to_str()
                              .ok_or(anyhow!("src not found"))?,
                          )
                          .join(&attr_src_path);
                        let dest_path = ctx.destination.join(&attr_src_path);

                        let dest_dir =
                          dest_path.parent().ok_or(anyhow!("destination not found"))?;

                        fs::create_dir_all(dest_dir)?;

                        fs::copy(src_path, dest_path)?;

                        writeln!(content_str, "#figure(\n  image(\"{}\")\n)", attr_src_path)?
                      }
                    }
                  }
                  "span" => (),
                  _ => (),
                }
              }
            }
          }
        }
      }
      Event::Text(t) => match event_stack.last() {
        Some(EventType::CodeBlock) => write!(content_str, "{}", t)?,
        Some(EventType::TableHead) => write!(content_str, "*{}*", t)?,
        Some(EventType::Image) => write!(content_str, "/* {} */", t)?,
        _ => {
          let mut transformed_text = String::with_capacity(t.len());
          for ch in t.chars() {
            match ch {
              '#' | '$' | '`' | '*' | '_' | '<' | '>' | '@' => {
                transformed_text.push('\\');
                transformed_text.push(ch);
              }
              _ => transformed_text.push(ch),
            }
          }

          write!(content_str, "{}", transformed_text)?
        }
      },
      Event::SoftBreak => writeln!(content_str)?,
      _ => (),
    }
  }

  Ok(content_str)
}
