#show raw.where(block: true): block.with(
  fill: luma(240),
  inset: 10pt,
  radius: 4pt,
)

#set page(
  header: locate(loc => {
    if counter(page).at(loc).first() > 1 [
      _Title_
    ]
  }),
  footer: locate(loc => {
    if counter(page).at(loc).first() > 1 [
      #counter(page).display(
        "1/1",
        both: true,
      )
    ]
  }),
)

#align(center, text(17pt)[
  *Title*
])

#pagebreak()
#set text(lang: "zh")
#outline(depth: 2, indent: 1em)
#pagebreak()

/**** MDBOOK_TYPST_PDF_PLACEHOLDER ****/
