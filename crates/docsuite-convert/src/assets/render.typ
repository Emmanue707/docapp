#let doc = json("doc.json")
#let pg = doc.page

#set page(
  width: pg.width_pt * 1pt,
  height: pg.height_pt * 1pt,
  margin: (
    top: pg.margin_top_pt * 1pt,
    right: pg.margin_right_pt * 1pt,
    bottom: pg.margin_bottom_pt * 1pt,
    left: pg.margin_left_pt * 1pt,
  ),
)

#let body-font = "DejaVu Sans"

#let run-styles(run) = (
  weight: if run.bold { "bold" } else { "regular" },
  style: if run.italic { "italic" } else { "normal" },
  size: if run.size_pt != none { run.size_pt * 1pt } else { 11pt },
  fill: if run.color != none { rgb(run.color) } else { black },
  font: if run.font_family != none { run.font_family } else { body-font },
)

#let render-runs(runs) = {
  for run in runs {
    let t = run.text
  text(..run-styles(run), t)
  }
}

#let render-paragraph(p) = {
  let body = render-runs(p.runs)
  if p.props.align == "center" {
    align(center)[#body]
  } else if p.props.align == "right" {
    align(right)[#body]
  } else {
    body
  }
}

#let render-table(t) = {
  table(
    columns: if t.col_widths_pt.len() > 0 { t.col_widths_pt.map(c => c * 1pt) } else { 1fr },
    stroke: if t.borders { 0.5pt + black } else { none },
    ..t.rows.map(row => row.cells.map(cell => {
      table.cell(colspan: cell.colspan, rowspan: cell.rowspan)[
        #for b in cell.blocks { render-block(b) }
      ]
    })).flatten()
  )
}

#let render-image(img) = {
  image(img.path, width: img.width_pt * 1pt, height: img.height_pt * 1pt)
}

#let render-block(block) = {
  if block.type == "paragraph" {
    let level = if block.style.type == "heading" { block.style.level } else { 0 }
    if level > 0 {
      text(size: (18pt - level * 1.5pt), weight: "bold")[#render-runs(block.runs)]
    } else if block.style.type == "list" {
      let prefix = if block.style.list_kind == "bullet" { "• " } else { "" }
      [#prefix#render-runs(block.runs)]
    } else {
      render-paragraph(block)
    }
  } else if block.type == "table" {
    render-table(block)
  } else if block.type == "image" {
    render-image(block)
  } else if block.type == "page_break" {
    pagebreak()
  }
}

#for block in doc.blocks {
  render-block(block)
}