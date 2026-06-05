//! Generates a landscape PDF listing publicly-accessible vehicles that already
//! hold SpaceX (Space Exploration Technologies Corp.) equity — i.e. ways to get
//! "pre-IPO" exposure today. SpaceX is private (it files only Form D), so the
//! exposure is disclosed in each holder's SEC filing: listed closed-end funds,
//! interval funds and mutual funds report their SpaceX stakes in Form
//! N-PORT / N-CSR. Each row links to the actual filing on sec.gov.
//!
//! Holdings were located via EDGAR full-text search for the exact phrase
//! "Space Exploration Technologies" (efts.sec.gov) and each filing line was
//! verified. Figures are as last disclosed and will change.

use ab_glyph::{Font as AbFont, FontRef, PxScale, ScaleFont};
use printpdf::{
    Actions, BorderArray, Color, ColorArray, Line, LinePoint, LinkAnnotation, Op, PaintMode,
    PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rect, Rgb,
    TextItem, WindingOrder,
};

const FONT_REGULAR: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf");

const MM: f32 = 2.834_645;
const PAGE_W: f32 = 297.0 * MM;
const PAGE_H: f32 = 210.0 * MM;
const MARGIN_X: f32 = 28.0;
const WRAP_FUDGE: f32 = 1.14;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, None))
}

struct Pal {
    header_bg: Color,
    header_fg: Color,
    row_a: Color,
    row_b: Color,
    border: Color,
    outer: Color,
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

    fn paragraph(&mut self, x: f32, top: f32, text: &str, size: f32, col: Color, max_w: f32) -> f32 {
        let by = |t: f32| PAGE_H - t;
        let lines = self.wrap(text, false, size, max_w);
        let mut y = top;
        for ln in &lines {
            self.line(x, by(y), ln, false, size, col.clone(), None);
            y += size + 2.4;
        }
        y
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_table(
        &mut self,
        x: f32,
        top: f32,
        widths: &[f32],
        headers: &[&str],
        rows: &[Vec<(String, bool, Color, Option<String>)>],
        pal: &Pal,
        cell: f32,
        head: f32,
        lh: f32,
        pad_x: f32,
        pad_y: f32,
    ) -> f32 {
        let by = |t: f32| PAGE_H - t;
        let table_w: f32 = widths.iter().sum();
        let ncol = widths.len();

        let head_wrap: Vec<Vec<String>> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| self.wrap(h, true, head, widths[i] - pad_x * 2.0))
            .collect();
        let head_lines = head_wrap.iter().map(|l| l.len()).max().unwrap_or(1) as f32;
        let head_h = head_lines * lh + pad_y * 2.0;

        let row_wrap: Vec<Vec<Vec<String>>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(i, (t, b, _, _))| self.wrap(t, *b, cell, widths[i] - pad_x * 2.0))
                    .collect()
            })
            .collect();
        let row_h: Vec<f32> = row_wrap
            .iter()
            .map(|r| r.iter().map(|c| c.len()).max().unwrap_or(1) as f32 * lh + pad_y * 2.0)
            .collect();
        let table_h = head_h + row_h.iter().sum::<f32>();

        self.fill_rect(x, by(top + head_h), table_w, head_h, pal.header_bg.clone());
        let mut ry = top + head_h;
        for (i, h) in row_h.iter().enumerate() {
            let c = if i % 2 == 0 { pal.row_a.clone() } else { pal.row_b.clone() };
            self.fill_rect(x, by(ry + h), table_w, *h, c);
            ry += h;
        }
        let mut cx = x;
        for (i, lines) in head_wrap.iter().enumerate() {
            for (li, ln) in lines.iter().enumerate() {
                self.line(cx + pad_x, by(top + pad_y + head + li as f32 * lh), ln, true, head, pal.header_fg.clone(), None);
            }
            cx += widths[i];
        }
        let mut ry = top + head_h;
        for (ri, r) in rows.iter().enumerate() {
            let mut cx = x;
            for (i, (_, b, col, url)) in r.iter().enumerate() {
                for (li, ln) in row_wrap[ri][i].iter().enumerate() {
                    self.line(cx + pad_x, by(ry + pad_y + cell + li as f32 * lh), ln, *b, cell, col.clone(), url.as_deref());
                }
                cx += widths[i];
            }
            ry += row_h[ri];
        }
        let g_top = by(top);
        let g_bot = by(top + table_h);
        let mut vx = x;
        self.seg(vx, g_top, vx, g_bot, 0.8, pal.outer.clone());
        for i in 0..ncol {
            vx += widths[i];
            let last = i == ncol - 1;
            self.seg(vx, g_top, vx, g_bot, if last { 0.8 } else { 0.5 }, if last { pal.outer.clone() } else { pal.border.clone() });
        }
        self.seg(x, g_top, x + table_w, g_top, 0.8, pal.outer.clone());
        let yh = top + head_h;
        self.seg(x, by(yh), x + table_w, by(yh), 0.8, pal.outer.clone());
        let mut ry = yh;
        for (i, h) in row_h.iter().enumerate() {
            ry += h;
            let last = i == row_h.len() - 1;
            self.seg(x, by(ry), x + table_w, by(ry), if last { 0.8 } else { 0.5 }, if last { pal.outer.clone() } else { pal.border.clone() });
        }
        top + table_h
    }
}

/// One holder of SpaceX equity that discloses it in an SEC filing.
struct Holder {
    /// Fund / company name and (where applicable) exchange ticker.
    name: &'static str,
    /// Vehicle type and how a public investor accesses it.
    vehicle: &'static str,
    /// SpaceX exposure as last disclosed in the linked filing.
    stake: &'static str,
    /// Source-link label (form + period).
    form: &'static str,
    /// EDGAR document URL.
    url: &'static str,
}

// --- Verified EDGAR filings (each line confirmed to name "Space Exploration Technologies"). ---
const DXYZ: &str = "https://www.sec.gov/Archives/edgar/data/1843974/000139834425011504/fp0093574-2_ncsrsa.htm";
const BARON_SELECT: &str = "https://www.sec.gov/Archives/edgar/data/1217673/000141036826049915/NPORT_FBBD_83396493_0326.htm";
const ARKVX: &str = "https://www.sec.gov/Archives/edgar/data/1905088/000121390026041819/ea0279145-01_ncsrs.htm";
const STEPSTONE: &str = "https://www.sec.gov/Archives/edgar/data/1918642/000119312526247111/primary_doc.xml";
const PIIVX: &str = "https://www.sec.gov/Archives/edgar/data/1557265/000119312526241036/tpsf_033126partf.htm";
const COATUE: &str = "https://www.sec.gov/Archives/edgar/data/2044519/000141036826056381/NPORT_CTIF_65685294_0326.htm";
const BARON_TRUST: &str = "https://www.sec.gov/Archives/edgar/data/810902/000141036826021330/NPORT_FBA4_46309207_1225.htm";
const BCAT: &str = "https://www.sec.gov/Archives/edgar/data/1809541/000175272425037817/primary_doc.xml";
// Alphabet's only by-name SpaceX disclosure is its 2015 filing (now folded into "non-marketable equity").
const GOOG_2015: &str = "https://www.sec.gov/Archives/edgar/data/1652044/000165204415000005/alpha10-qq32015.htm";

fn main() {
    let mut warns = Vec::new();
    let reg = printpdf::ParsedFont::from_bytes(FONT_REGULAR, 0, &mut warns).expect("reg");
    let bold = printpdf::ParsedFont::from_bytes(FONT_BOLD, 0, &mut warns).expect("bold");
    let mut doc = PdfDocument::new("Public-Market Exposure to a SpaceX IPO — SEC-Filed Holders");
    let reg_id = doc.add_font(&reg);
    let bold_id = doc.add_font(&bold);
    let mut pdf = Pdf {
        ops: Vec::new(),
        reg: PdfFontHandle::External(reg_id),
        bold: PdfFontHandle::External(bold_id),
        reg_ab: FontRef::try_from_slice(FONT_REGULAR).unwrap(),
        bold_ab: FontRef::try_from_slice(FONT_BOLD).unwrap(),
    };

    let header_bg = rgb(30, 58, 95);
    let header_fg = rgb(255, 255, 255);
    let row_a = rgb(255, 255, 255);
    let row_b = rgb(237, 242, 248);
    let ink = rgb(30, 41, 59);
    let company_c = rgb(12, 74, 110);
    let stake_c = rgb(21, 101, 52);
    let note_c = rgb(71, 85, 105);
    let link_c = rgb(13, 71, 161);
    let gray = rgb(100, 116, 139);
    let title_c = rgb(15, 23, 42);
    let site_c = rgb(55, 48, 107);

    let pal = Pal {
        header_bg: header_bg.clone(),
        header_fg: header_fg.clone(),
        row_a: row_a.clone(),
        row_b: row_b.clone(),
        border: rgb(203, 213, 225),
        outer: rgb(148, 163, 184),
    };

    let by = |t: f32| PAGE_H - t;

    // ---- Title ----
    let mut top = 24.0f32;
    pdf.line(MARGIN_X, by(top), "How to Own SpaceX Before the IPO — Public Funds That Already Hold It (per SEC filings)", true, 15.0, title_c.clone(), None);
    top += 15.0;
    top = pdf.paragraph(MARGIN_X, top, "SpaceX is private and files only Form D. Listed closed-end funds, interval funds and mutual funds disclose their SpaceX stakes in Form N-PORT / N-CSR — each row links to the actual filing. Ordered by SpaceX share of the fund.", 9.0, gray.clone(), PAGE_W - 2.0 * MARGIN_X);
    top += 2.0;

    // Geometry shared with the data-center PDF.
    let cell = 7.6f32;
    let head = 7.8f32;
    let lh = 9.4f32;
    let pad_x = 4.0f32;
    let pad_y = 4.8f32;

    let headers = [
        "Holder (ticker)",
        "Vehicle — how a public investor gets exposure",
        "SpaceX stake, as last disclosed in the filing",
        "SEC filing (linked)",
    ];
    let col_w = [156.0f32, 206.0, 244.0, 180.0]; // 786
    let table_x = (PAGE_W - col_w.iter().sum::<f32>()) / 2.0;

    let holders = [
        Holder {
            name: "Destiny Tech100  (NYSE: DXYZ)",
            vehicle: "Listed closed-end fund — trades like a stock; built as a basket of top private companies, SpaceX is its anchor.",
            stake: "~37% of net assets via SpaceX SPVs (DXYZ SpaceX I 27.3% + MWAM VC SpaceX-II 8.3% + Celadon 1.8%) — by far its largest position.",
            form: "N-CSRS/A · 3/31/2025 \u{2197}",
            url: DXYZ,
        },
        Holder {
            name: "Baron Partners Fund  (BPTRX)",
            vehicle: "Mutual fund (Baron Select Funds) — buy directly or via most brokerages.",
            stake: "SpaceX is the single largest holding — ~12.8% of net assets on the Class A line ($1.17B), with additional preferred-series lines.",
            form: "NPORT-EX · 3/31/2026 \u{2197}",
            url: BARON_SELECT,
        },
        Holder {
            name: "Baron Focused Growth  (BFGFX)",
            vehicle: "Mutual fund (Baron Select Funds) — same filing as Baron Partners.",
            stake: "Top holding — SpaceX ~14% of net assets ($386M, Class A line) plus other classes.",
            form: "NPORT-EX · 3/31/2026 \u{2197}",
            url: BARON_SELECT,
        },
        Holder {
            name: "ARK Venture Fund  (ARKVX)",
            vehicle: "Continuously-offered interval fund (ARK) — low minimum, sold through brokerages/Titan.",
            stake: "One of its largest positions; held directly across several SpaceX series (common + preferred).",
            form: "N-CSRS · filed Apr 2026 \u{2197}",
            url: ARKVX,
        },
        Holder {
            name: "StepStone Private Venture & Growth Fund",
            vehicle: "Interval fund (StepStone) — private-markets access for qualified investors.",
            stake: "~9.1% of net assets across three SpaceX lines ($117.0M + $307.1M + $162.4M).",
            form: "NPORT-P · 3/31/2026 \u{2197}",
            url: STEPSTONE,
        },
        Holder {
            name: "The Private Shares Fund  (PIIVX)",
            vehicle: "Interval fund (Liberty Street) — late-stage private companies; available at brokerages.",
            stake: "SpaceX held directly — ~$190.5M fair value (cost $28.7M), a long-held position marked up sharply.",
            form: "NPORT-EX · 3/31/2026 \u{2197}",
            url: PIIVX,
        },
        Holder {
            name: "Coatue Innovative Strategies Fund",
            vehicle: "Interval fund (Coatue) — tech-focused private/public crossover.",
            stake: "SpaceX Class A ~1.7% of net assets ($85.1M), plus additional share-class lines.",
            form: "NPORT-EX · 3/31/2026 \u{2197}",
            url: COATUE,
        },
        Holder {
            name: "Baron Asset / Baron Growth Funds",
            vehicle: "Mutual funds (Baron Investment Funds Trust) — separate trust from Baron Select.",
            stake: "Smaller SpaceX positions incl. a Series N preferred line (~$405M); not a top holding of these funds.",
            form: "NPORT-EX · 12/31/2025 \u{2197}",
            url: BARON_TRUST,
        },
        Holder {
            name: "BlackRock Capital Allocation Term Trust  (NYSE: BCAT)",
            vehicle: "Listed closed-end fund — trades like a stock; diversified multi-asset, small private sleeve.",
            stake: "A small SpaceX position among hundreds of holdings — exposure is minor relative to NAV.",
            form: "NPORT-P \u{2197}",
            url: BCAT,
        },
    ];

    let rows: Vec<Vec<(String, bool, Color, Option<String>)>> = holders
        .iter()
        .map(|h| {
            vec![
                (h.name.to_string(), true, company_c.clone(), None),
                (h.vehicle.to_string(), false, ink.clone(), None),
                (h.stake.to_string(), false, stake_c.clone(), None),
                (h.form.to_string(), false, link_c.clone(), Some(h.url.to_string())),
            ]
        })
        .collect();

    let table_bottom = pdf.draw_table(table_x, top + 6.0, &col_w, &headers, &rows, &pal, cell, head, lh, pad_x, pad_y);

    // ---- Footnotes ----
    let fw = PAGE_W - 2.0 * MARGIN_X;
    let mut f = table_bottom + 16.0;
    f = pdf.paragraph(MARGIN_X, f, "Purest listed proxy: Destiny Tech100 (DXYZ) — the only exchange-traded fund whose dominant exposure is SpaceX, so it trades partly as a SpaceX tracker (often at a large premium/discount to NAV). BlackRock's BCAT is the other NYSE-listed vehicle here, but its SpaceX weight is tiny.", 7.6, site_c.clone(), fw) + 3.0;
    f = pdf.paragraph(MARGIN_X, f, "Diversified fund families also hold small SpaceX positions (well under ~1% of NAV each), confirmed in EDGAR full-text search but immaterial per fund: Fidelity (Contrafund, Blue Chip Growth and others), Neuberger Berman, and Franklin Strategic Series. They are not a meaningful way to bet on a SpaceX IPO.", 7.6, gray.clone(), fw) + 3.0;
    f = pdf.paragraph(MARGIN_X, f, "Operating-company caveat — Alphabet (GOOGL): its $900M SpaceX investment (January 2015) is named only in 2015-era filings; current 10-Ks fold it anonymously into \"non-marketable equity securities\" at cost, so Alphabet gives no transparent, sized SpaceX exposure today. SpaceX's own EDGAR file (CIK 1181412) contains only Form D notices.", 7.6, note_c.clone(), fw);
    pdf.line(MARGIN_X, by(f), "Alphabet Q3-2015 10-Q (the $900M SpaceX investment) \u{2197}", false, 7.6, link_c.clone(), Some(GOOG_2015));
    f += 7.6 + 5.4;
    pdf.paragraph(MARGIN_X, f, "Method: holdings located via SEC EDGAR full-text search (efts.sec.gov) for the exact phrase \"Space Exploration Technologies\", then each filing line verified. Figures are as last disclosed (mostly 3/31/2026 N-PORT) and change every quarter; fund NAVs mark SpaceX to estimated fair value, not a market price. Not investment advice.", 7.6, gray.clone(), fw);

    let page1 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    doc.with_pages(vec![page1]);
    let mut sw: Vec<printpdf::PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut sw);
    std::fs::write("pdf/spacex_exposure.pdf", &bytes).expect("write pdf");
    println!("Wrote pdf/spacex_exposure.pdf ({} bytes)", bytes.len());
}
