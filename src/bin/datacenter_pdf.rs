//! Generates a landscape PDF table (mirroring the PNG chart) of data-center
//! power capacity. Each row carries one source hyperlink in the Capex column:
//! public companies link to their FY2025 10-K on sec.gov, private companies to
//! their primary public announcement. Alphabet adds a second link (the FWP for
//! its $80B AI-infra equity raise — a distinct SEC document).

use ab_glyph::{Font as AbFont, FontRef, PxScale, ScaleFont};
use printpdf::{
    Actions, BorderArray, Color, ColorArray, Line, LinePoint, LinkAnnotation, Op, PaintMode,
    PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rect, Rgb,
    TextItem, WindingOrder,
};

const FONT_REGULAR: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf");

// A4 landscape in points (1 mm = 2.834645 pt).
const MM: f32 = 2.834_645;
const PAGE_W: f32 = 297.0 * MM; // 841.9
const PAGE_H: f32 = 210.0 * MM; // 595.3
const MARGIN_X: f32 = 28.0;

// ab_glyph slightly under-measures poppler's rendered advance; inflate when
// wrapping so a line never overflows its cell.
const WRAP_FUDGE: f32 = 1.14;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, None))
}

struct Pdf<'a> {
    ops: Vec<Op>,
    reg: PdfFontHandle,
    bold: PdfFontHandle,
    reg_ab: FontRef<'a>,
    bold_ab: FontRef<'a>,
}

impl<'a> Pdf<'a> {
    fn width(&self, text: &str, bold: bool, size: f32) -> f32 {
        let font = if bold { &self.bold_ab } else { &self.reg_ab };
        let scaled = font.as_scaled(PxScale::from(size));
        let mut w = 0.0f32;
        let mut prev = None;
        for c in text.chars() {
            let gid = font.glyph_id(c);
            if let Some(p) = prev {
                w += scaled.kern(font.glyph_id(p), gid);
            }
            w += scaled.h_advance(gid);
            prev = Some(c);
        }
        w
    }

    fn wrap(&self, text: &str, bold: bool, size: f32, max_w: f32) -> Vec<String> {
        let mut out = Vec::new();
        for segment in text.split('\n') {
            let mut line = String::new();
            for word in segment.split_whitespace() {
                let cand = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", line, word)
                };
                if self.width(&cand, bold, size) * WRAP_FUDGE <= max_w || line.is_empty() {
                    line = cand;
                } else {
                    out.push(line);
                    line = word.to_string();
                }
            }
            out.push(line);
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }

    /// One left-aligned line at absolute baseline `by` (from page bottom).
    /// If `url` is set, the line is drawn as an underlined hyperlink.
    fn line(&mut self, x: f32, by: f32, text: &str, bold: bool, size: f32, col: Color, url: Option<&str>) {
        let handle = if bold { self.bold.clone() } else { self.reg.clone() };
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetFont { font: handle, size: Pt(size) });
        self.ops.push(Op::SetFillColor { col: col.clone() });
        self.ops.push(Op::SetTextCursor { pos: Point { x: Pt(x), y: Pt(by) } });
        self.ops.push(Op::ShowText { items: vec![TextItem::Text(text.to_string())] });
        self.ops.push(Op::EndTextSection);

        if let Some(url) = url {
            let w = self.width(text, bold, size);
            self.ops.push(Op::SetOutlineColor { col: col.clone() });
            self.ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
            self.ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint { p: Point { x: Pt(x), y: Pt(by - 1.3) }, bezier: false },
                        LinePoint { p: Point { x: Pt(x + w), y: Pt(by - 1.3) }, bezier: false },
                    ],
                    is_closed: false,
                },
            });
            self.ops.push(Op::LinkAnnotation {
                link: LinkAnnotation::new(
                    Rect {
                        x: Pt(x),
                        y: Pt(by - 2.5),
                        width: Pt(w),
                        height: Pt(size + 2.5),
                        mode: None,
                        winding_order: None,
                    },
                    Actions::Uri(url.to_string()),
                    Some(BorderArray::Solid([0.0, 0.0, 0.0])),
                    Some(ColorArray::Transparent),
                    None,
                ),
            });
        }
    }

    /// Filled rectangle given lower-left (x, y) from page bottom.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: Color) {
        self.ops.push(Op::SetFillColor { col });
        self.ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint { p: Point { x: Pt(x), y: Pt(y) }, bezier: false },
                        LinePoint { p: Point { x: Pt(x + w), y: Pt(y) }, bezier: false },
                        LinePoint { p: Point { x: Pt(x + w), y: Pt(y + h) }, bezier: false },
                        LinePoint { p: Point { x: Pt(x), y: Pt(y + h) }, bezier: false },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        });
    }

    fn seg(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, col: Color) {
        self.ops.push(Op::SetOutlineColor { col });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thick) });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point { x: Pt(x0), y: Pt(y0) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x1), y: Pt(y1) }, bezier: false },
                ],
                is_closed: false,
            },
        });
    }
}

struct Row {
    company: &'static str,
    operational: &'static str,
    planned: &'static str,
    capex: &'static str,
    /// (label, url) links shown beneath the capex figure (usually one).
    links: &'static [(&'static str, &'static str)],
    sites: &'static str,
    notes: &'static str,
}

const AMZN: &str = "https://www.sec.gov/Archives/edgar/data/1018724/000101872426000004/amzn-20251231.htm";
const MSFT: &str = "https://www.sec.gov/Archives/edgar/data/789019/000095017025100235/msft-20250630.htm";
const GOOG: &str = "https://www.sec.gov/Archives/edgar/data/1652044/000165204426000018/goog-20251231.htm";
const GOOG_FWP: &str = "https://www.sec.gov/Archives/edgar/data/1652044/000119312526251733/d160205dfwp.htm";
const META: &str = "https://www.sec.gov/Archives/edgar/data/1326801/000162828026003942/meta-20251231.htm";
const XAI: &str = "https://x.ai/news/anthropic-compute-partnership";
const OPENAI: &str = "https://openai.com/index/five-new-stargate-sites/";
const ANTHROPIC: &str = "https://www.anthropic.com/news/anthropic-invests-50-billion-in-american-ai-infrastructure";

fn main() {
    let mut warns = Vec::new();
    let reg = printpdf::ParsedFont::from_bytes(FONT_REGULAR, 0, &mut warns).expect("reg");
    let bold = printpdf::ParsedFont::from_bytes(FONT_BOLD, 0, &mut warns).expect("bold");
    let mut doc = PdfDocument::new("Data Center Power Capacity — Operational vs. Planned, with SEC Capex");
    let reg_id = doc.add_font(&reg);
    let bold_id = doc.add_font(&bold);
    let mut pdf = Pdf {
        ops: Vec::new(),
        reg: PdfFontHandle::External(reg_id),
        bold: PdfFontHandle::External(bold_id),
        reg_ab: FontRef::try_from_slice(FONT_REGULAR).unwrap(),
        bold_ab: FontRef::try_from_slice(FONT_BOLD).unwrap(),
    };

    // Palette
    let header_bg = rgb(30, 58, 95);
    let header_fg = rgb(255, 255, 255);
    let row_a = rgb(255, 255, 255);
    let row_b = rgb(237, 242, 248);
    let ink = rgb(30, 41, 59);
    let company_c = rgb(12, 74, 110);
    let capex_c = rgb(21, 101, 52);
    let site_c = rgb(55, 48, 107);
    let note_c = rgb(71, 85, 105);
    let link_c = rgb(13, 71, 161);
    let border = rgb(203, 213, 225);
    let outer = rgb(148, 163, 184);
    let title_c = rgb(15, 23, 42);
    let gray = rgb(100, 116, 139);

    let headers = [
        "Company",
        "Operational (GW)",
        "Planned / Under Construction",
        "FY2025 Capex (10-K)",
        "Key Sites & Power (location · GW)",
        "Key Notes",
    ];
    let col_w = [80.0f32, 92.0, 118.0, 80.0, 208.0, 208.0]; // sum = 786
    let table_w: f32 = col_w.iter().sum();
    let table_x = (PAGE_W - table_w) / 2.0;

    let rows = [
        Row {
            company: "Amazon (AWS)",
            operational: "~10–15+ GW (global est.)",
            planned: "Multi-GW additions ongoing (on track to ~2× capacity by 2027)",
            capex: "$128.3B",
            links: &[("10-K ↗", AMZN)],
            sites: "New Carlisle, IN ($11–15B; ~2.4 GW added in region) · N. Virginia (~2.75 GW; $35B through 2040)",
            notes: "Added 3.8 GW in past 12 mo. 2.2 GW Indiana campus partly operational. 10-K: capex expected to increase in 2026.",
        },
        Row {
            company: "Microsoft (Azure)",
            operational: "~5–8+ GW (global est.)",
            planned: "Large pipeline (multi-GW projects)",
            capex: "$64.6B",
            links: &[("10-K ↗", MSFT)],
            sites: "Fairwater — Wisconsin (~$7.3B; ~0.9 GW, online early 2026) · Atlanta (online) · Fairwater 4 (under constr.)",
            notes: "Added ~2 GW FY2025 + ~1 GW Q2 FY2026. FY ends June. 10-K: will continue to invest in AI infrastructure.",
        },
        Row {
            company: "Google (Cloud)",
            operational: "Several GW (global est.)",
            planned: "Significant expansions (e.g., 1 GW+ demand-response deals)",
            capex: "$91.4B",
            links: &[("10-K ↗", GOOG), ("FWP ↗", GOOG_FWP)],
            sites: "Global fleet. $52.7B of long-term data-center leases signed but not yet commenced (10-K)",
            notes: "10-K: expects to significantly increase 2026 technical-infra spend. AI-infra financing: $80B equity raise (Jun 2026, incl. $10B Berkshire) — see FWP.",
        },
        Row {
            company: "Meta",
            operational: "Several GW operational",
            planned: "Prometheus ~1 GW (2026); Hyperion →5 GW (2 GW by ~2030)",
            capex: "$69.7B",
            links: &[("10-K ↗", META)],
            sites: "Prometheus — New Albany, OH (~1 GW, online 2026) · Hyperion — Richland Parish, LA (→5 GW; $27B Blue Owl JV)",
            notes: "10-K guides FY2026 capex to ~$115–135B. Hyperion is among the largest planned campuses worldwide.",
        },
        Row {
            company: "xAI",
            operational: "~2 GW (Colossus, Memphis)",
            planned: "Further expansions (roadmap to much larger)",
            capex: "Private",
            links: &[("source ↗", XAI)],
            sites: "Memphis, TN — Colossus 1 (~0.3 GW) + Colossus 2 (~1.2 → ~2 GW, 555k+ GPUs); power hub in Southaven, MS",
            notes: "Colossus 2 among first ~GW-scale single sites. Colossus 1 output leased to Anthropic ($1.25B/mo through 2029).",
        },
        Row {
            company: "OpenAI",
            operational: "~0.3 GW (Stargate Abilene) + Azure",
            planned: "Stargate ~7–10 GW planned ($500B); 4.5 GW Oracle deal",
            capex: "Private",
            links: &[("source ↗", OPENAI)],
            sites: "Abilene, TX (→1.2 GW; ~0.3 GW live) · Shackelford Co., TX · Doña Ana Co., NM · Lordstown, OH · Wisconsin · UAE",
            notes: "Stargate JV with SoftBank & Oracle (+ CoreWeave). Targets ~10 GW / $500B by 2029; >3 GW added in early 2026.",
        },
        Row {
            company: "Anthropic",
            operational: "Limited owned (partner access)",
            planned: "Multi-GW via partners (1+ GW coming 2026–2027)",
            capex: "Private",
            links: &[("source ↗", ANTHROPIC)],
            sites: "Fluidstack: Abernathy, TX (~168 MW) · Lake Mariner, NY (~360 MW). Partners: AWS, Google, Azure, xAI",
            notes: "$50B US infrastructure plan, sites online through 2026. Exploring multi-GW orbital compute with SpaceX.",
        },
    ];

    // ---- Title ----
    let mut top = 24.0f32; // baseline distance from page top
    let by = |t: f32| PAGE_H - t;
    pdf.line(MARGIN_X, by(top), "Data Center Power Capacity (GW) — Operational vs. Planned, with SEC Capex", true, 15.0, title_c.clone(), None);
    top += 15.0;
    pdf.line(MARGIN_X, by(top), "As of mid-2026 · capex from FY2025 SEC 10-K filings · rows ordered by estimated operational GW", false, 9.0, gray.clone(), None);
    top += 12.0;

    // ---- Table geometry ----
    let cell = 7.3f32;
    let head = 7.6f32;
    let lh = 9.0f32; // line height
    let pad_x = 4.0f32;
    let pad_y = 4.5f32;
    let table_top = top + 6.0;

    // Header height (wrap header titles too).
    let header_wrapped: Vec<Vec<String>> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| pdf.wrap(h, true, head, col_w[i] - pad_x * 2.0))
        .collect();
    let header_lines = header_wrapped.iter().map(|l| l.len()).max().unwrap_or(1) as f32;
    let header_h = header_lines * lh + pad_y * 2.0;

    // Pre-wrap each data cell; row height from the tallest cell.
    let col_color = [company_c.clone(), ink.clone(), ink.clone(), capex_c.clone(), site_c.clone(), note_c.clone()];
    let col_bold = [true, false, false, true, false, false];

    let mut row_data: Vec<([Vec<String>; 6], f32)> = Vec::new();
    for r in &rows {
        let texts = [r.company, r.operational, r.planned, r.capex, r.sites, r.notes];
        let wrapped: [Vec<String>; 6] = std::array::from_fn(|i| {
            pdf.wrap(texts[i], col_bold[i], cell, col_w[i] - pad_x * 2.0)
        });
        // capex cell also carries link lines beneath the figure.
        let capex_lines = wrapped[3].len() + r.links.len();
        let max_lines = wrapped
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 3 { capex_lines } else { w.len() })
            .max()
            .unwrap_or(1) as f32;
        let h = max_lines * lh + pad_y * 2.0;
        row_data.push((wrapped, h));
    }
    let table_h = header_h + row_data.iter().map(|(_, h)| h).sum::<f32>();

    // ---- Draw fills (header + alternating rows) ----
    pdf.fill_rect(table_x, by(table_top + header_h), table_w, header_h, header_bg.clone());
    let mut ry = table_top + header_h;
    for (i, (_, h)) in row_data.iter().enumerate() {
        let c = if i % 2 == 0 { row_a.clone() } else { row_b.clone() };
        pdf.fill_rect(table_x, by(ry + h), table_w, *h, c);
        ry += h;
    }

    // ---- Header text ----
    {
        let mut cx = table_x;
        for (i, lines) in header_wrapped.iter().enumerate() {
            for (li, ln) in lines.iter().enumerate() {
                let bl = by(table_top + pad_y + head + li as f32 * lh);
                pdf.line(cx + pad_x, bl, ln, true, head, header_fg.clone(), None);
            }
            cx += col_w[i];
        }
    }

    // ---- Row text ----
    let mut ry = table_top + header_h;
    for (ri, r) in rows.iter().enumerate() {
        let (wrapped, h) = &row_data[ri];
        let mut cx = table_x;
        for i in 0..6 {
            for (li, ln) in wrapped[i].iter().enumerate() {
                let bl = by(ry + pad_y + cell + li as f32 * lh);
                pdf.line(cx + pad_x, bl, ln, col_bold[i], cell, col_color[i].clone(), None);
            }
            // Links under the capex figure.
            if i == 3 {
                let base = wrapped[3].len();
                for (k, (label, url)) in r.links.iter().enumerate() {
                    let bl = by(ry + pad_y + cell + (base + k) as f32 * lh);
                    pdf.line(cx + pad_x, bl, label, false, cell, link_c.clone(), Some(url));
                }
            }
            cx += col_w[i];
        }
        ry += *h;
    }

    // ---- Grid lines ----
    let t_top = by(table_top);
    let t_bot = by(table_top + table_h);
    // verticals
    let mut vx = table_x;
    pdf.seg(vx, t_top, vx, t_bot, 0.8, outer.clone());
    for i in 0..6 {
        vx += col_w[i];
        let c = if i == 5 { outer.clone() } else { border.clone() };
        pdf.seg(vx, t_top, vx, t_bot, if i == 5 { 0.8 } else { 0.5 }, c);
    }
    // horizontals
    pdf.seg(table_x, t_top, table_x + table_w, t_top, 0.8, outer.clone());
    let yh = table_top + header_h;
    pdf.seg(table_x, by(yh), table_x + table_w, by(yh), 0.8, outer.clone());
    let mut ry = yh;
    for (i, (_, h)) in row_data.iter().enumerate() {
        ry += h;
        let last = i == row_data.len() - 1;
        pdf.seg(table_x, by(ry), table_x + table_w, by(ry), if last { 0.8 } else { 0.5 }, if last { outer.clone() } else { border.clone() });
    }

    // ---- Footer ----
    let foot = table_top + table_h + 16.0;
    pdf.line(MARGIN_X, by(foot), "Capex = purchases of property & equipment (latest annual 10-K). Click a source link in the Capex column to open the filing. GW figures are press/analyst-sourced — SEC filings do not disclose capacity in gigawatts.", false, 7.2, gray.clone(), None);

    // ---- Save ----
    let page = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), pdf.ops);
    doc.with_pages(vec![page]);
    let mut sw: Vec<printpdf::PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut sw);
    std::fs::write("pdf/datacenter_sources.pdf", &bytes).expect("write pdf");
    println!("Wrote pdf/datacenter_sources.pdf ({} bytes)", bytes.len());
}
