# mdbook-typst-pdf

[中文版说明](README-cn.md)

A [mdBook](https://github.com/rust-lang/mdBook) backend for generate pdf (through [typst](https://github.com/typst/typst)).

For now the primary use case is convert [Rust 程序设计语言 简体中文版](https://kaisery.github.io/trpl-zh-cn) to PDF. It should work for other mdbook project, if not welcome to fire an issue.

## Installation

- `cargo install mdbook-typst-pdf`
- Or download from [releases](https://github.com/KaiserY/mdbook-typst-pdf/releases)

## Usage

Add follow `[output.typst-pdf]` section to `book.toml` then `mdbook build`

```toml
[book]
...

[output.html]
...

[output.typst-pdf]
pdf = true # false for generate typ file only
custom_template = "template.typ" # filename for custom typst template for advanced styling
section-number = true # true for generate chapter head numbering
chapter_no_pagebreak = true # true for not add pagebreak after chapter
```

## Custom template

see [src/assets/template.typ](https://github.com/KaiserY/mdbook-typst-pdf/blob/main/src/assets/template.typ) file for more details, for now there are two placeholders:

- `MDBOOK_TYPST_PDF_TITLE` for title
- `/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/` for content

## Demo PDF

[Rust 程序设计语言 简体中文版.pdf](https://kaisery.github.io/trpl-zh-cn/Rust%20%E7%A8%8B%E5%BA%8F%E8%AE%BE%E8%AE%A1%E8%AF%AD%E8%A8%80%20%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87%E7%89%88.pdf)

## Related projects

- https://github.com/typst/typst
- https://github.com/rust-lang/mdBook
- https://github.com/lbeckman314/mdbook-latex
- https://github.com/LegNeato/mdbook-typst
