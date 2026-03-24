use anyhow::anyhow;
use comrak::Anchorizer;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use mdbook_renderer::RenderContext;
use mdbook_renderer::book::BookItem;
use mdbook_renderer::book::Chapter;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::fmt::Write;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use crate::Config;

static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, PartialEq)]
pub enum EventType {
  CodeBlockIndented,
  CodeBlockFenced(String),
  List,
  NumberedList,
  TableHead,
  Image,
  RemoteImage,
  Heading,
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

  let placeholder = "/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/";
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
    if cfg.chapter_no_pagebreak {
      writeln!(book_item_str, "{}", convert_content(ctx, cfg, ch)?)?;
    } else {
      writeln!(
        book_item_str,
        "{}#pagebreak(weak: true)",
        convert_content(ctx, cfg, ch)?
      )?;
    }
  }

  Ok(book_item_str)
}

fn convert_content(
  ctx: &RenderContext,
  cfg: &Config,
  ch: &Chapter,
) -> Result<String, anyhow::Error> {
  let label_path = ch
    .source_path
    .to_owned()
    .ok_or(anyhow!("source_path not found"))?;

  let label = label_path
    .as_path()
    .file_name()
    .and_then(|f| f.to_str())
    .and_then(|f| f.split('.').next())
    .ok_or(anyhow!("label not found"))?;

  let mut content_str = String::new();
  let chapter_dir = ch
    .source_path
    .as_ref()
    .and_then(|path| path.parent())
    .map(|path| {
      ctx
        .root
        .join(
          ctx
            .config
            .book
            .src
            .to_str()
            .expect("book src should be valid utf-8"),
        )
        .join(path)
    });
  let chapter_rel_dir = label_path.parent();

  let mut heading = String::new();

  let mut writen_invisible_heading = false;

  let mut options = Options::ENABLE_SMART_PUNCTUATION
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_FOOTNOTES
    | Options::ENABLE_TASKLISTS
    | Options::ENABLE_TABLES;

  if cfg.enable_math {
    options |= Options::ENABLE_MATH;
  }

  let parser = Parser::new_ext(&ch.content, options);

  let mut event_stack = Vec::new();

  let email_regex: &Regex = EMAIL_REGEX
    .get_or_init(|| Regex::new(r"(?i)^\w+([\.-]?\w+)*@\w+([\.-]?\w+)*(.\w{2,3})+$").unwrap());

  let mut anchorizer = Anchorizer::new();

  for event in parser {
    match event {
      Event::Start(Tag::Heading { level, .. }) => {
        event_stack.push(EventType::Heading);

        heading.clear();

        let level_usize: usize = level as usize;

        write!(
          content_str,
          "#heading(level: {}, outlined: false, bookmarked: false)[",
          level_usize,
        )?;
      }
      Event::End(TagEnd::Heading(level)) => {
        event_stack.pop();

        let level_usize: usize = level as usize;

        writeln!(
          content_str,
          "] <{}.html-{}>",
          label,
          anchorizer.anchorize(&heading)
        )?;

        if !writen_invisible_heading {
          let invisible_heading = if let Some(number) = &ch.number {
            if cfg.section_number {
              format!(
                "#{{\n  show heading: none\n  heading(numbering: none, level: {}, outlined: true, bookmarked: true)[#\"{} {}\"]\n}} <{}.html>",
                number.len(),
                number,
                ch.name,
                label,
              )
            } else {
              format!(
                "#{{\n  show heading: none\n  heading(numbering: none, level: {}, outlined: true, bookmarked: true)[{}]\n}} <{}.html>",
                level_usize, ch.name, label
              )
            }
          } else {
            format!(
              "#{{\n  show heading: none\n  heading(numbering: none, level: 1, outlined: true, bookmarked: true)[{}]\n}} <{}.html>",
              ch.name, label,
            )
          };

          writeln!(content_str, "{}", invisible_heading)?;

          writen_invisible_heading = true;
        }
      }
      Event::Start(Tag::Emphasis) => write!(content_str, "#emph[")?,
      Event::End(TagEnd::Emphasis) => write!(content_str, "]/**/")?,
      Event::Start(Tag::Strong) => write!(content_str, "#strong[")?,
      Event::End(TagEnd::Strong) => write!(content_str, "]/**/")?,
      Event::Start(Tag::Strikethrough) => write!(content_str, "#strong[")?,
      Event::End(TagEnd::Strikethrough) => write!(content_str, "]/**/")?,
      Event::Start(Tag::BlockQuote(_)) => write!(content_str, "#quote(block: true)[")?,
      Event::End(TagEnd::BlockQuote(_)) => writeln!(content_str, "]")?,
      Event::Start(Tag::List(None)) => {
        event_stack.push(EventType::List);
      }
      Event::Start(Tag::List(Some(_))) => {
        event_stack.push(EventType::NumberedList);
      }
      Event::End(TagEnd::List(_)) => {
        event_stack.pop();
      }
      Event::Start(Tag::Item) => match event_stack.last() {
        Some(EventType::List) => write!(content_str, "- ")?,
        Some(EventType::NumberedList) => write!(content_str, "+ ")?,
        _ => write!(content_str, "- ")?,
      },
      Event::End(TagEnd::Item) => writeln!(content_str)?,
      Event::Start(Tag::Paragraph) => (),
      Event::End(TagEnd::Paragraph) => write!(content_str, "\n\n")?,
      Event::Start(Tag::Link { dest_url, .. }) => {
        if cfg.rust_book {
          if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
            write!(content_str, "#link(\"{}\")[", dest_url)?
          } else if email_regex.is_match(&dest_url) {
            write!(content_str, "#link(\"mailto:{}\")[", dest_url)?
          } else if dest_url.starts_with('#') {
            write!(
              content_str,
              "#link(<{}.html{}>)[",
              label,
              dest_url.replace('#', "-")
            )?
          } else {
            write!(content_str, "#link(<{}>)[", dest_url.replace('#', "-"))?
          }
        } else {
          if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
            write!(content_str, "#link(\"{}\")[", dest_url)?
          } else if email_regex.is_match(&dest_url) {
            write!(content_str, "#link(\"mailto:{}\")[", dest_url)?
          } else {
            write!(
              content_str,
              "#link(\"{}\")[",
              normalize_relative_link(chapter_rel_dir, &dest_url)
            )?
          }
        }
      }
      Event::End(TagEnd::Link) => write!(content_str, "]")?,
      Event::Start(Tag::Table(align)) => {
        let typst_align = align
          .iter()
          .map(|a| match a {
            Alignment::None => "auto",
            Alignment::Left => "left + horizon",
            Alignment::Center => "center + horizon",
            Alignment::Right => "right + horizon",
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
      Event::End(TagEnd::Table) => writeln!(content_str, ")")?,
      Event::Start(Tag::TableHead) => {
        event_stack.push(EventType::TableHead);
      }
      Event::End(TagEnd::TableHead) => {
        event_stack.pop();
      }
      Event::Start(Tag::TableRow) => (),
      Event::End(TagEnd::TableRow) => (),
      Event::Start(Tag::TableCell) => write!(content_str, "[")?,
      Event::End(TagEnd::TableCell) => writeln!(content_str, "],")?,
      Event::Start(Tag::Image { dest_url, .. }) => {
        if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
          event_stack.push(EventType::RemoteImage);
          continue;
        }

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
          .join(dest_url.to_string());
        let src_path = if src_path.exists() {
          src_path
        } else if let Some(chapter_dir) = &chapter_dir {
          let chapter_src_path = chapter_dir.join(dest_url.to_string());
          if chapter_src_path.exists() {
            chapter_src_path
          } else {
            src_path
          }
        } else {
          src_path
        };
        let output_path = normalize_output_path(chapter_rel_dir, &dest_url);
        let dest_path = ctx.destination.join(&output_path);

        let dest_dir = dest_path.parent().ok_or(anyhow!("destination not found"))?;

        fs::create_dir_all(dest_dir)?;

        if !dest_path.exists() {
          fs::copy(src_path, dest_path)?;
        }

        write!(
          content_str,
          "#figure(\n  image(\"{}\")\n)",
          output_path.display()
        )?
      }
      Event::End(TagEnd::Image) => match event_stack.pop() {
        Some(EventType::Image) => writeln!(content_str)?,
        Some(EventType::RemoteImage) => (),
        _ => (),
      },
      Event::Start(Tag::CodeBlock(ref lang)) => match lang {
        CodeBlockKind::Indented => {
          event_stack.push(EventType::CodeBlockIndented);

          writeln!(content_str, "````")?
        }
        CodeBlockKind::Fenced(lang) => {
          event_stack.push(EventType::CodeBlockFenced(lang.to_string()));

          let langs: Vec<&str> = lang.split(',').collect();

          if !langs.is_empty() {
            let mut ferris_prefix = "".to_string();

            for l in langs.iter().skip(1) {
              match l {
                &"does_not_compile" | &"not_desired_behavior" | &"panics" => {
                  ferris_prefix = "#columns(1)[\n".to_string();
                }
                _ => (),
              }
            }

            writeln!(content_str, "{}````{}", ferris_prefix, langs[0])?
          } else {
            writeln!(content_str, "````")?
          }
        }
      },
      Event::End(TagEnd::CodeBlock) => {
        match event_stack.last() {
          Some(EventType::CodeBlockIndented) => writeln!(content_str, "````")?,
          Some(EventType::CodeBlockFenced(lang)) => {
            let langs: Vec<&str> = lang.split(',').collect();

            if !langs.is_empty() {
              let mut ferris_suffix = "".to_string();

              for l in langs.iter().skip(1) {
                match l {
                  &"does_not_compile" | &"not_desired_behavior" | &"panics" => {
                    let ferris_src_path = format!("img/ferris/{}.svg", l);

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
                      .join(&ferris_src_path);

                    let dest_path = ctx.destination.join(&ferris_src_path);

                    let dest_dir = dest_path.parent().ok_or(anyhow!("destination not found"))?;

                    fs::create_dir_all(dest_dir)?;

                    if !dest_path.exists() {
                      fs::copy(src_path, dest_path)?;
                    }

                    ferris_suffix = format!(
                      "\n#place(\n  top + right,\n  figure(\n    image(\"{}\", width: 10%)\n  )\n)\n]",
                      ferris_src_path
                    );
                  }
                  _ => (),
                }
              }

              writeln!(content_str, "````{}", ferris_suffix)?
            } else {
              writeln!(content_str, "````")?
            }
          }
          _ => writeln!(content_str, "````")?,
        }

        event_stack.pop();
      }
      Event::Code(t) => {
        if event_stack.contains(&EventType::Heading) {
          heading.push_str(&t);
        }

        write!(
          content_str,
          r#"#raw("{}")/**/"#,
          t.replace('\\', r#"\\"#).replace('"', r#"\""#)
        )?;
      }
      Event::Html(t) | Event::InlineHtml(t) => {
        match t.trim() {
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

        if !dom_children.is_empty() && matches!(dom_children[0].data, NodeData::Element { .. }) {
          let html_children = &dom_children[0].children.borrow();

          if html_children.len() > 1 {
            let body_children = &html_children[1].children.borrow();

            if !body_children.is_empty()
              && let NodeData::Element { name, attrs, .. } = &body_children[0].data
            {
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

                      let dest_dir = dest_path.parent().ok_or(anyhow!("destination not found"))?;

                      fs::create_dir_all(dest_dir)?;

                      if !dest_path.exists() {
                        fs::copy(src_path, dest_path)?;
                      }

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
      Event::InlineMath(t) => {
        if looks_like_non_math(t.trim()) {
          write!(content_str, "\\${}\\$", escape_typst_text(t.trim()))?;
        } else {
          writeln!(content_str, "${}$", t.trim())?;
        }
      }
      Event::DisplayMath(t) => {
        if looks_like_non_math(t.trim()) {
          write!(content_str, "\\${}\\$", escape_typst_text(t.trim()))?;
        } else {
          writeln!(content_str, "$  {}  $", t.trim())?;
        }
      }
      Event::Text(t) => {
        if event_stack.contains(&EventType::Heading) {
          heading.push_str(&t);
        }

        match event_stack.last() {
          Some(EventType::CodeBlockIndented) => write!(content_str, "{}", t)?,
          Some(EventType::CodeBlockFenced(_)) => {
            if cfg.rust_book {
              write!(content_str, "{}", strip_rust_book_hidden_lines(&t))?
            } else {
              write!(content_str, "{}", t)?
            }
          }
          Some(EventType::TableHead) => write!(content_str, "*{}*", t)?,
          Some(EventType::Image) => write!(content_str, "/* {} */", t)?,
          Some(EventType::RemoteImage) => {
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
        }
      }
      Event::SoftBreak => writeln!(content_str)?,
      _ => (),
    }
  }

  Ok(content_str)
}

fn escape_typst_text(text: &str) -> String {
  let mut transformed_text = String::with_capacity(text.len());
  for ch in text.chars() {
    match ch {
      '#' | '$' | '`' | '*' | '_' | '<' | '>' | '@' => {
        transformed_text.push('\\');
        transformed_text.push(ch);
      }
      _ => transformed_text.push(ch),
    }
  }

  transformed_text
}

fn looks_like_non_math(text: &str) -> bool {
  text.contains('"')
    || text.contains('\'')
    || text.contains(';')
    || text.contains("${")
    || text.contains("sprintf")
    || text.contains("file_name")
    || text.contains("index)")
    || text.contains("slide_time")
}

fn strip_rust_book_hidden_lines(block: &str) -> String {
  let mut output = String::with_capacity(block.len());

  for line in block.split_inclusive('\n') {
    let (line, newline) = match line.strip_suffix('\n') {
      Some(line) => (line, "\n"),
      None => (line, ""),
    };

    if line == "#" || line.starts_with("# ") {
      continue;
    }

    if let Some(unescaped) = line.strip_prefix("##") {
      output.push('#');
      output.push_str(unescaped);
    } else {
      output.push_str(line);
    }

    output.push_str(newline);
  }

  output
}

fn normalize_relative_link(chapter_rel_dir: Option<&Path>, dest_url: &str) -> String {
  let normalized = normalize_output_path(chapter_rel_dir, dest_url);
  let normalized = normalized.to_string_lossy();

  if let Some((path, fragment)) = dest_url.split_once('#') {
    if path.ends_with(".md") {
      let normalized_path = normalized_output_path_str(chapter_rel_dir, path);
      format!(
        "{}.html-{}",
        &normalized_path[..normalized_path.len() - 3],
        fragment
      )
    } else {
      format!("{}-{}", normalized, fragment)
    }
  } else if let Some(stripped) = normalized.strip_suffix(".md") {
    format!("{stripped}.html")
  } else {
    normalized.into_owned()
  }
}

fn normalize_output_path(chapter_rel_dir: Option<&Path>, target: &str) -> PathBuf {
  let base = if Path::new(target).is_absolute() {
    PathBuf::new()
  } else {
    chapter_rel_dir.map(Path::to_path_buf).unwrap_or_default()
  };

  normalize_join(base.join(target))
}

fn normalized_output_path_str(chapter_rel_dir: Option<&Path>, target: &str) -> String {
  normalize_output_path(chapter_rel_dir, target)
    .to_string_lossy()
    .into_owned()
}

fn normalize_join(path: PathBuf) -> PathBuf {
  let mut normalized = PathBuf::new();

  for component in path.components() {
    match component {
      Component::CurDir => (),
      Component::ParentDir => {
        normalized.pop();
      }
      Component::Normal(part) => normalized.push(part),
      Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
      Component::RootDir => (),
    }
  }

  normalized
}

#[cfg(test)]
mod tests {
  use super::strip_rust_book_hidden_lines;

  #[test]
  fn strips_hidden_rust_book_lines() {
    let input = "# #[derive(Debug)]\n# struct Point;\n#\np1.distance(&p2);\n";
    let expected = "p1.distance(&p2);\n";

    assert_eq!(strip_rust_book_hidden_lines(input), expected);
  }

  #[test]
  fn unescapes_visible_hash_lines() {
    let input = "##[derive(Debug)]\n## heading\n";
    let expected = "#[derive(Debug)]\n# heading\n";

    assert_eq!(strip_rust_book_hidden_lines(input), expected);
  }
}
