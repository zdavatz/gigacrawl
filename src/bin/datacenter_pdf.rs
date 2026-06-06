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

    /// Emit a left-aligned, word-wrapped paragraph starting at baseline `top`
    /// (from page top); returns the `top` just past the last line.
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

    /// Render a self-contained table (header band + alternating rows + grid)
    /// at absolute `top` (from page top). Each cell is (text, bold, color,
    /// optional link URL). Returns the `top` coordinate of the table's bottom.
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

        // Fills
        self.fill_rect(x, by(top + head_h), table_w, head_h, pal.header_bg.clone());
        let mut ry = top + head_h;
        for (i, h) in row_h.iter().enumerate() {
            let c = if i % 2 == 0 { pal.row_a.clone() } else { pal.row_b.clone() };
            self.fill_rect(x, by(ry + h), table_w, *h, c);
            ry += h;
        }
        // Header text
        let mut cx = x;
        for (i, lines) in head_wrap.iter().enumerate() {
            for (li, ln) in lines.iter().enumerate() {
                self.line(cx + pad_x, by(top + pad_y + head + li as f32 * lh), ln, true, head, pal.header_fg.clone(), None);
            }
            cx += widths[i];
        }
        // Row text
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
        // Grid
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

struct Row {
    company: &'static str,
    cost_gw: &'static str,
    operational: &'static str,
    planned: &'static str,
    capex: &'static str,
    /// (label, url) links shown beneath the capex figure (usually one).
    links: &'static [(&'static str, &'static str)],
    sites: &'static str,
    notes: &'static str,
}

/// Shared palette passed to `Pdf::draw_table`.
struct Pal {
    header_bg: Color,
    header_fg: Color,
    row_a: Color,
    row_b: Color,
    border: Color,
    outer: Color,
}

/// One public company's audited FY2025 annual-report figures (page 2).
struct Sec {
    company: &'static str,
    capex23: &'static str,
    capex24: &'static str,
    capex25: &'static str,
    ppe: &'static str,
    ocf: &'static str,
    ratio: &'static str,
    leases: &'static str,
    url: &'static str,
    /// Source-link label — "10-K ↗" for domestic filers, "20-F ↗" for Nebius.
    form: &'static str,
}

const AMZN: &str = "https://www.sec.gov/Archives/edgar/data/1018724/000101872426000004/amzn-20251231.htm";
const MSFT: &str = "https://www.sec.gov/Archives/edgar/data/789019/000095017025100235/msft-20250630.htm";
const GOOG: &str = "https://www.sec.gov/Archives/edgar/data/1652044/000165204426000018/goog-20251231.htm";
const GOOG_FWP: &str = "https://www.sec.gov/Archives/edgar/data/1652044/000119312526251733/d160205dfwp.htm";
const META: &str = "https://www.sec.gov/Archives/edgar/data/1326801/000162828026003942/meta-20251231.htm";
const ORCL: &str = "https://www.sec.gov/Archives/edgar/data/1341439/000095017025087926/orcl-20250531.htm";
// Nebius is a foreign private issuer — files Form 20-F (US GAAP), not 10-K.
const NBIS: &str = "https://www.sec.gov/Archives/edgar/data/1513845/000110465926052948/nbis-20251231x20f.htm";
const CRWV: &str = "https://www.sec.gov/Archives/edgar/data/1769628/000176962826000104/crwv-20251231.htm";
// Behind-the-meter / off-grid build-out: press/analyst, not in any SEC filing.
const CLEANVIEW: &str = "https://cleanview.co/reports/behind-the-meter-data-centers";
const XAI: &str = "https://x.ai/news/anthropic-compute-partnership";
// SpaceX IPO Free Writing Prospectus (Rule 433, File 333-296070, filed 5 Jun 2026):
// discloses the Google Cloud Service Agreement — $920M/mo, ~110k Nvidia GPUs,
// Oct 2026–Jun 2029. The first xAI/SpaceX compute contract to appear in an SEC filing.
const SPACEX_FWP: &str = "https://www.sec.gov/Archives/edgar/data/1181412/000162828026041150/spacexagreementfwp.htm";
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
        "FY2025 Capex (SEC)",
        "Est. $/GW (flagship)",
        "Key Sites & Power (location · GW)",
        "Key Notes",
    ];
    let col_w = [72.0f32, 84.0, 110.0, 64.0, 78.0, 189.0, 189.0]; // sum = 786
    let table_w: f32 = col_w.iter().sum();
    let table_x = (PAGE_W - table_w) / 2.0;

    let rows = [
        Row {
            company: "Amazon (AWS)",
            operational: "~10–15+ GW (global est.)",
            planned: "Multi-GW additions ongoing (on track to ~2× capacity by 2027)",
            capex: "$128.3B",
            cost_gw: "~$5–6B/GW (facility)",
            links: &[("10-K ↗", AMZN)],
            sites: "New Carlisle, IN ($11–15B; ~2.4 GW; ~500k AWS Trainium2 — Project Rainier) · N. Virginia (~2.75 GW; $35B through 2040)",
            notes: "Added 3.8 GW in past 12 mo. 2.2 GW Indiana campus partly operational. 10-K: capex expected to increase in 2026.",
        },
        Row {
            company: "Microsoft (Azure)",
            operational: "~5–8+ GW (global est.)",
            planned: "Large pipeline (multi-GW projects)",
            capex: "$64.6B",
            cost_gw: "~$8B/GW (facility)",
            links: &[("10-K ↗", MSFT)],
            sites: "Fairwater — Wisconsin (~$7.3B; ~0.9 GW, early 2026) · Atlanta (online; GB300 NVL72, Blackwell Ultra) · Fairwater 4 (constr.)",
            notes: "Added ~2 GW FY2025 + ~1 GW Q2 FY2026. FY ends June. 10-K: will continue to invest in AI infrastructure.",
        },
        Row {
            company: "Google (Cloud)",
            operational: "Several GW (global est.)",
            planned: "Significant expansions (e.g., 1 GW+ demand-response deals)",
            capex: "$91.4B",
            cost_gw: "— (n/d)",
            links: &[("10-K ↗", GOOG), ("FWP ↗", GOOG_FWP)],
            sites: "Global fleet (TPU v7 Ironwood + Nvidia). $52.7B of long-term data-center leases signed but not yet commenced (10-K)",
            notes: "10-K: expects to significantly increase 2026 technical-infra spend. AI-infra financing: $80B equity raise (Jun 2026, incl. $10B Berkshire) — see FWP.",
        },
        Row {
            company: "Meta",
            operational: "Several GW operational",
            planned: "Prometheus ~1 GW (2026); Hyperion →5 GW (2 GW by ~2030)",
            capex: "$69.7B",
            cost_gw: "— (n/d)",
            links: &[("10-K ↗", META)],
            sites: "Prometheus — New Albany, OH (~1 GW, 2026; Blackwell GB200/GB300) · Hyperion — Richland Parish, LA (→5 GW; $27B Blue Owl JV)",
            notes: "10-K guides FY2026 capex to ~$115–135B. Hyperion is among the largest planned campuses worldwide.",
        },
        Row {
            company: "Oracle (OCI)",
            operational: "~2–3 GW (OCI global est.)",
            planned: ">10 GW power secured for next 3 yrs; 4.5 GW Stargate deal (OpenAI)",
            capex: "$21.2B",
            cost_gw: "— (n/d)",
            links: &[("10-K ↗", ORCL)],
            sites: "Abilene, TX (Stargate flagship; ~1.2 GW, →~450k GB200 — built by Crusoe, OCI operates) · Shackelford & Doña Ana Co. · Wisconsin (Vantage) · Michigan",
            notes: "RPO backlog $553B (Q3 FY2026), mostly large AI contracts. FY2026 capex guided ~$50B. Reported ~$300B/5-yr OpenAI compute deal. FY ends May.",
        },
        Row {
            company: "CoreWeave (CRWV)",
            operational: "~1 GW active (→>1.7 GW by end-2026; 43 data centers)",
            planned: ">3.5 GW contracted; 5+ GW w/ Nvidia by 2030 — heavily leased",
            capex: "$10.31B",
            cost_gw: "— (n/d)",
            links: &[("10-K ↗", CRWV)],
            sites: "43 leased data centers (US + Europe). First to deploy GB300 NVL72; first Vera Rubin NVL72 bring-up (Jun 2026, with Dell)",
            notes: "Neocloud — rents Nvidia GPU capacity; leases its DCs, funded by ~$21B debt (9%+ notes). OpenAI ~$22.4B & Meta ~$35B contracts; Nvidia $6.3B take-or-pay backstop. Microsoft ~62% of revenue. RPO $60.7B (10-K). Net loss $1.2B. Ex-crypto miner; IPO Mar 2025.",
        },
        Row {
            company: "xAI",
            operational: "~0.8 GW live, ~2 GW total/building (Colossus, Memphis)",
            planned: "Further expansions (roadmap to much larger)",
            capex: "Private",
            cost_gw: "~$9–15B/GW (all-in)",
            links: &[("source ↗", XAI), ("FWP ↗", SPACEX_FWP)],
            sites: "Memphis, TN — Colossus 1 (~0.3 GW; ~230k: 150k H100/50k H200/30k GB200) + Colossus 2 (→~555k GPUs, mostly GB200); power hub in Southaven, MS",
            notes: "Colossus 2 among first ~GW-scale single sites. Compute leased out: Anthropic $1.25B/mo (Colossus 1, thru 2029) + Google $920M/mo for ~110k GPUs, Oct 2026–Jun 2029 (SpaceX IPO FWP, 5 Jun 2026 — first such contract in an SEC filing).",
        },
        Row {
            company: "Nebius (NBIS)",
            operational: "~0.5 GW connected (→0.8–1 GW by end-2026)",
            planned: ">3.5 GW contracted (→>4 GW by end-2026); >75% owned",
            capex: "$4.07B",
            cost_gw: "— (n/d)",
            links: &[("20-F ↗", NBIS)],
            sites: "Owned ~3 GW / 5 sites: Independence, MO (1.2 GW) · Vineland, NJ (Microsoft) · Pennsylvania (→1.2 GW, 2027) · Alabama (2027) · Finland (310 MW). GB300 NVL72; early Vera Rubin",
            notes: "Neocloud — rents Nvidia GPU capacity. Meta deal up to $27B/5 yr; Microsoft up to $19.4B thru 2031. RPO backlog $21.3B (20-F). FY ends Dec. Ex-Yandex; on Nasdaq since Oct 2024.",
        },
        Row {
            company: "OpenAI",
            operational: "~0.3 GW (Stargate Abilene) + Azure",
            planned: "Stargate ~7–10 GW planned ($500B); 4.5 GW Oracle deal",
            capex: "Private",
            cost_gw: "~$50B/GW (all-in)",
            links: &[("source ↗", OPENAI)],
            sites: "Abilene, TX (→1.2 GW; ~0.3 GW live; 450k GB200) · Shackelford Co., TX · Doña Ana Co., NM · Lordstown, OH · Wisconsin · UAE",
            notes: "Stargate JV with SoftBank & Oracle (+ CoreWeave). Targets ~10 GW / $500B by 2029; >3 GW added in early 2026.",
        },
        Row {
            company: "Anthropic",
            operational: "Limited owned (partner access)",
            planned: "Multi-GW via partners (1+ GW coming 2026–2027)",
            capex: "Private",
            cost_gw: "— (n/d)",
            links: &[("source ↗", ANTHROPIC)],
            sites: "Fluidstack: Abernathy, TX (~168 MW) · Lake Mariner, NY (~360 MW). Partners: AWS Trainium2 (~500k→1M), Google TPU v7 (≤1M), Azure (Nvidia), xAI Colossus 1",
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
    let costgw_c = rgb(146, 64, 14);
    let col_color = [
        company_c.clone(),
        ink.clone(),
        ink.clone(),
        capex_c.clone(),
        costgw_c.clone(),
        site_c.clone(),
        note_c.clone(),
    ];
    let col_bold = [true, false, false, true, true, false, false];

    let mut row_data: Vec<([Vec<String>; 7], f32)> = Vec::new();
    for r in &rows {
        let texts = [r.company, r.operational, r.planned, r.capex, r.cost_gw, r.sites, r.notes];
        let wrapped: [Vec<String>; 7] = std::array::from_fn(|i| {
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
        for i in 0..7 {
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
    for i in 0..7 {
        vx += col_w[i];
        let c = if i == 6 { outer.clone() } else { border.clone() };
        pdf.seg(vx, t_top, vx, t_bot, if i == 6 { 0.8 } else { 0.5 }, c);
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
    let fw1 = PAGE_W - 2.0 * MARGIN_X;
    let mut foot = table_top + table_h + 14.0;
    foot = pdf.paragraph(MARGIN_X, foot, "Capex = purchases of property & equipment (latest annual 10-K, or 20-F for Nebius). Click a source link in the Capex column to open the filing. GW figures are press/analyst-sourced — SEC filings do not disclose capacity in gigawatts.", 7.2, gray.clone(), fw1) + 2.0;
    pdf.paragraph(MARGIN_X, foot, "Est. $/GW = flagship-project cost ÷ that project's power. \"facility\" excludes IT (industry ~$8–12B/GW); \"all-in\" includes GPUs/servers (~$35–60B/GW; Nvidia cites $50–60B). \"n/d\" = no per-project cost disclosed. Press/analyst-derived, not an SEC figure.", 7.2, gray.clone(), fw1);

    // Page 1 done — capture its ops.
    let page1 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ===================== PAGE 2: SEC financials =====================
    // Sorted by FY2025 capex, descending.
    let secs = [
        Sec { company: "Amazon (AWS)", capex23: "$52.7B", capex24: "$83.0B", capex25: "$131.8B",
              ppe: "$357.0B  (+41%)", ocf: "$139.5B", ratio: "94%", leases: "$96.4B", url: AMZN, form: "10-K ↗" },
        Sec { company: "Alphabet (Google)", capex23: "$32.3B", capex24: "$52.5B", capex25: "$91.4B",
              ppe: "$246.6B  (+44%)", ocf: "$164.7B", ratio: "56%", leases: "$58.5B", url: GOOG, form: "10-K ↗" },
        Sec { company: "Meta Platforms", capex23: "$27.3B", capex24: "$37.3B", capex25: "$69.7B",
              ppe: "$176.4B  (+45%)", ocf: "$115.8B", ratio: "60%", leases: "$103.8B", url: META, form: "10-K ↗" },
        Sec { company: "Microsoft (Azure)", capex23: "$28.1B", capex24: "$44.5B", capex25: "$64.6B",
              ppe: "$205.0B  (+51%)", ocf: "$136.2B", ratio: "47%", leases: "$92.7B", url: MSFT, form: "10-K ↗" },
        Sec { company: "Oracle (OCI)", capex23: "$8.7B", capex24: "$6.9B", capex25: "$21.2B",
              ppe: "$43.5B  (+102%)", ocf: "$20.8B", ratio: "102%", leases: "$43.4B", url: ORCL, form: "10-K ↗" },
        Sec { company: "CoreWeave (CRWV)", capex23: "$2.94B", capex24: "$8.70B", capex25: "$10.31B",
              ppe: "$30.56B  (+156%)", ocf: "$3.06B", ratio: "337%", leases: "$38.5B", url: CRWV, form: "10-K ↗" },
        Sec { company: "Nebius (NBIS)", capex23: "$0.08B", capex24: "$0.81B", capex25: "$4.07B",
              ppe: "$5.55B  (+556%)", ocf: "$0.38B", ratio: "1057%", leases: "$9.76B", url: NBIS, form: "20-F ↗" },
    ];
    let s_head = [
        "Company",
        "Capex FY23",
        "Capex FY24",
        "Capex FY25",
        "PP&E net (Δ YoY)",
        "Op. cash flow FY25",
        "Capex ÷ OCF",
        "Leases not yet commenced (mostly data centers)",
        "Source",
    ];
    let s_w = [96.0f32, 66.0, 66.0, 74.0, 96.0, 84.0, 60.0, 156.0, 88.0]; // 786
    let s_color = [
        company_c.clone(), ink.clone(), ink.clone(), capex_c.clone(), ink.clone(),
        ink.clone(), capex_c.clone(), site_c.clone(), link_c.clone(),
    ];
    let s_bold = [true, false, false, true, true, false, true, false, true];
    let s_x = (PAGE_W - s_w.iter().sum::<f32>()) / 2.0;

    let mut t2 = 24.0f32;
    pdf.line(MARGIN_X, by(t2), "FY2025 SEC Financials — AI Data-Center Capex & Commitments", true, 15.0, title_c.clone(), None);
    t2 += 15.0;
    t2 = pdf.paragraph(MARGIN_X, t2, "Audited figures from each company's latest annual report on SEC EDGAR (Form 10-K, or 20-F for Nebius), sorted by FY2025 capex. Private operators (xAI, OpenAI, Anthropic) file no SEC reports and are omitted.", 9.0, gray.clone(), PAGE_W - 2.0 * MARGIN_X) + 2.0;

    let s_top = t2 + 6.0;
    let s_head_wrap: Vec<Vec<String>> = s_head.iter().enumerate()
        .map(|(i, h)| pdf.wrap(h, true, head, s_w[i] - pad_x * 2.0)).collect();
    let s_head_lines = s_head_wrap.iter().map(|l| l.len()).max().unwrap_or(1) as f32;
    let s_head_h = s_head_lines * lh + pad_y * 2.0;

    let s_rowtexts: Vec<[&str; 9]> = secs.iter().map(|s| {
        [s.company, s.capex23, s.capex24, s.capex25, s.ppe, s.ocf, s.ratio, s.leases, s.form]
    }).collect();
    let s_wrapped: Vec<[Vec<String>; 9]> = s_rowtexts.iter().map(|t| {
        std::array::from_fn(|i| pdf.wrap(t[i], s_bold[i], cell, s_w[i] - pad_x * 2.0))
    }).collect();
    let s_heights: Vec<f32> = s_wrapped.iter()
        .map(|w| w.iter().map(|c| c.len()).max().unwrap_or(1) as f32 * lh + pad_y * 2.0)
        .collect();
    let s_table_h = s_head_h + s_heights.iter().sum::<f32>();
    let s_table_w: f32 = s_w.iter().sum();

    // Fills
    pdf.fill_rect(s_x, by(s_top + s_head_h), s_table_w, s_head_h, header_bg.clone());
    let mut ry = s_top + s_head_h;
    for (i, h) in s_heights.iter().enumerate() {
        let c = if i % 2 == 0 { row_a.clone() } else { row_b.clone() };
        pdf.fill_rect(s_x, by(ry + h), s_table_w, *h, c);
        ry += h;
    }
    // Header text
    {
        let mut cx = s_x;
        for (i, lines) in s_head_wrap.iter().enumerate() {
            for (li, ln) in lines.iter().enumerate() {
                pdf.line(cx + pad_x, by(s_top + pad_y + head + li as f32 * lh), ln, true, head, header_fg.clone(), None);
            }
            cx += s_w[i];
        }
    }
    // Row text
    let mut ry = s_top + s_head_h;
    for (ri, s) in secs.iter().enumerate() {
        let mut cx = s_x;
        for i in 0..9 {
            let url = if i == 8 { Some(s.url) } else { None };
            for (li, ln) in s_wrapped[ri][i].iter().enumerate() {
                pdf.line(cx + pad_x, by(ry + pad_y + cell + li as f32 * lh), ln, s_bold[i], cell, s_color[i].clone(), url);
            }
            cx += s_w[i];
        }
        ry += s_heights[ri];
    }
    // Grid
    let g_top = by(s_top);
    let g_bot = by(s_top + s_table_h);
    let mut vx = s_x;
    pdf.seg(vx, g_top, vx, g_bot, 0.8, outer.clone());
    for i in 0..9 {
        vx += s_w[i];
        let last = i == 8;
        pdf.seg(vx, g_top, vx, g_bot, if last { 0.8 } else { 0.5 }, if last { outer.clone() } else { border.clone() });
    }
    pdf.seg(s_x, g_top, s_x + s_table_w, g_top, 0.8, outer.clone());
    let yh = s_top + s_head_h;
    pdf.seg(s_x, by(yh), s_x + s_table_w, by(yh), 0.8, outer.clone());
    let mut ry = yh;
    for (i, h) in s_heights.iter().enumerate() {
        ry += h;
        let last = i == s_heights.len() - 1;
        pdf.seg(s_x, by(ry), s_x + s_table_w, by(ry), if last { 0.8 } else { 0.5 }, if last { outer.clone() } else { border.clone() });
    }
    // Footnotes (word-wrapped)
    let fw = PAGE_W - 2.0 * MARGIN_X;
    let mut f2 = s_top + s_table_h + 16.0;
    f2 = pdf.paragraph(MARGIN_X, f2, "FY = fiscal year (Microsoft's ends June 30, Oracle's May 31; Amazon, Alphabet, Meta, CoreWeave & Nebius end December 31). Nebius is a foreign private issuer — it files Form 20-F (US GAAP), not 10-K. Capex = purchases of property & equipment from the cash-flow statement.", 7.2, gray.clone(), fw) + 2.0;
    f2 = pdf.paragraph(MARGIN_X, f2, "PP&E (property, plant & equipment), net = book value of long-lived physical assets (land, buildings, servers, network gear) after depreciation; Alphabet & Meta include finance-lease right-of-use assets.", 7.2, gray.clone(), fw) + 2.0;
    f2 = pdf.paragraph(MARGIN_X, f2, "\"Leases not yet commenced\" = signed future lease obligations not on the balance sheet, mostly data centers (10-K notes). Each figure is in the linked 10-K.", 7.2, gray.clone(), fw) + 2.0;
    f2 = pdf.paragraph(MARGIN_X, f2, "Capex ÷ OCF (operating cash flow) shows how much of the cash each firm generates from operations it reinvests in property & equipment.", 7.2, gray.clone(), fw) + 2.0;
    pdf.paragraph(MARGIN_X, f2, "Read leverage, not just the ratio: a low capex÷OCF can mask heavy debt. CoreWeave (337%) carries ~$21.4B of debt and a $1.2B net loss, while Nebius (1057%) holds ~$4B debt, >$9B cash, and is profitable — the reverse of what the ratio alone suggests.", 7.2, site_c.clone(), fw);

    // Page 2 done (SEC financials) — capture its ops.
    let page2 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ===================== PAGE 3: PP&E composition — compute/servers vs. real estate =====================
    let pal = Pal {
        header_bg: header_bg.clone(),
        header_fg: header_fg.clone(),
        row_a: row_a.clone(),
        row_b: row_b.clone(),
        border: border.clone(),
        outer: outer.clone(),
    };
    let mut t3 = 24.0f32;
    pdf.line(MARGIN_X, by(t3), "Where the capital sits — compute/servers vs. real estate (FY2025 gross PP&E)", true, 15.0, title_c.clone(), None);
    t3 += 15.0;
    t3 = pdf.paragraph(MARGIN_X, t3, "The audited equipment-vs-plant split that page 1's $/GW estimates approximate, from each SEC filing. GPUs sit in the \"compute\" bucket; SEC filings do not isolate GPU spend.", 9.0, gray.clone(), PAGE_W - 2.0 * MARGIN_X) + 2.0;

    let comp_head = [
        "Company",
        "Compute / server equipment (gross)",
        "Real estate (buildings + land)",
        "Construction in progress",
        "Finance-lease ROU (leased)",
        "Source",
    ];
    let comp_w = [110.0f32, 172.0, 152.0, 110.0, 122.0, 120.0]; // 786
    // Ordered by FY2025 capex (matches the table above).
    let comp_cells: [(&str, &str, &str, &str, &str, &str); 7] = [
        ("Amazon (AWS)", "$172.5B  servers & networking", "$155.1B  land & buildings", "$71.7B", "in land & bldg¹", AMZN),
        ("Alphabet (Google)", "~$122B  \u{2248}60% of tech. infra²", "~$82B DC bldg + $48.3B office²", "$78.6B  not yet in service", "embedded²", GOOG),
        ("Meta Platforms", "$98.0B  servers & network assets", "$59.3B  buildings + land", "$50.5B", "$8.2B", META),
        ("Microsoft (Azure)", "$132.8B  computer equip. & sw", "$159.4B  bldg + land + leasehold", "\u{2014}³", "$44.0B  (net)", MSFT),
        ("Oracle (OCI)", "$30.3B  computer, network & equip.", "$10.9B buildings + $1.4B land", "$16.5B  (mostly DC compute)\u{2074}", "$2.9B  (in PP&E, net)", ORCL),
        ("CoreWeave (CRWV)", "$20.9B  technology (GPU) equip.", "leases its data centers\u{2076}", "$9.38B  construction in progress", "$0.44B fin. / $8.23B op. ROU\u{2076}", CRWV),
        ("Nebius (NBIS)", "$3.12B  server & network equip.", "$0.38B buildings + land", "$2.42B  assets not yet in use", "none\u{2075}  (op. leases only)", NBIS),
    ];
    let comp_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = comp_cells
        .iter()
        .map(|(co, compute, re, cip, fl, url)| {
            let label = if co.starts_with("Nebius") { "20-F \u{2197}" } else { "10-K \u{2197}" };
            vec![
                (co.to_string(), true, company_c.clone(), None),
                (compute.to_string(), true, capex_c.clone(), None),
                (re.to_string(), false, ink.clone(), None),
                (cip.to_string(), false, ink.clone(), None),
                (fl.to_string(), false, ink.clone(), None),
                (label.to_string(), false, link_c.clone(), Some(url.to_string())),
            ]
        })
        .collect();
    let comp_bottom = pdf.draw_table(s_x, t3 + 4.0, &comp_w, &comp_head, &comp_rows, &pal, cell, head, lh, pad_x, pad_y);

    let mut cf = comp_bottom + 14.0;
    cf = pdf.paragraph(MARGIN_X, cf, "All figures are FY2025 gross (at cost) from each filing's property & equipment note, except finance-lease right-of-use assets (net). Category labels differ by filer; the \"compute\" column is each company's own server/equipment bucket — SEC filings do not isolate GPU spend.", 7.2, gray.clone(), fw) + 2.0;
    cf = pdf.paragraph(MARGIN_X, cf, "¹ Amazon reports land and buildings as one line (finance-lease property included within it) and its PP&E also holds large non-data-center fulfilment/logistics assets (heavy & other equipment $128.9B), so its compute share understates data-center intensity.", 7.2, gray.clone(), fw) + 2.0;
    cf = pdf.paragraph(MARGIN_X, cf, "² Alphabet's 10-K states ~60% of \"technical infrastructure\" ($203.7B) is servers & network equipment (\u{2248}$122B); the rest (\u{2248}$82B) is data-center land/buildings. Office space ($48.3B) is separate; finance-lease ROU is embedded in PP&E, not itemized.", 7.2, gray.clone(), fw) + 2.0;
    cf = pdf.paragraph(MARGIN_X, cf, "³ Microsoft presents PP&E at cost with no construction-in-progress line ($32.1B committed for datacenter/building construction at fiscal year-end); its $44.0B net finance-lease right-of-use assets are reported in the leases note, not in the PP&E table.", 7.2, gray.clone(), fw) + 2.0;
    cf = pdf.paragraph(MARGIN_X, cf, "\u{2074} Oracle (FY ended May 31, 2025): gross PP&E $59.6B; its construction-in-progress \"primarily consist[s] of computer equipment to be built and deployed at our data centers\" (10-K), so most of the $16.5B is compute. Finance-lease ROU ($2.9B net) sits inside PP&E; operating-lease ROU ($13.1B) is in other assets, and ~$43.4B of leases had not yet commenced.", 7.2, gray.clone(), fw) + 2.0;
    cf = pdf.paragraph(MARGIN_X, cf, "\u{2075} Nebius (FY2025 20-F, US GAAP): gross PP&E $6.19B. \"Assets not yet in use\" ($2.42B) is its construction-in-progress proxy. It reports operating leases only (ROU $0.92B) — no finance leases — plus ~$9.76B of leases not yet commenced (undiscounted), the SEC-filed proxy for its data-center pipeline.", 7.2, gray.clone(), fw) + 2.0;
    pdf.paragraph(MARGIN_X, cf, "\u{2076} CoreWeave (FY2025 10-K) leases its data centers — operating-lease ROU $8.23B, owning little real estate — so its GAAP finance leases are small ($0.44B). Its leverage is debt, not leases: ~$21.4B of borrowings (GPU-collateralized term loans + 9%+ senior notes), plus $38.5B of leases not yet commenced. FY2025 RPO (backlog) $60.7B; net loss $1.17B, driven by interest on that debt.", 7.2, gray.clone(), fw);

    // Page 3 done (PP&E composition) — capture its ops.
    let page3 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ===================== PAGE 4: private operators (estimates) =====================
    let mut t4 = 24.0f32;
    pdf.line(MARGIN_X, by(t4), "Private operators — GPU/silicon vs. construction, power & land", true, 15.0, title_c.clone(), None);
    t4 += 15.0;
    t4 = pdf.paragraph(MARGIN_X, t4, "xAI, OpenAI and Anthropic file no SEC reports. The figures below are company announcements or press/analyst ESTIMATES — not audited. They are shown apart from the SEC pages on purpose.", 9.0, gray.clone(), PAGE_W - 2.0 * MARGIN_X) + 2.0;

    let priv_head = [
        "Company / flagship",
        "GPUs / silicon",
        "Construction, power & land (plant)",
        "Split basis (press/analyst estimate)",
    ];
    let priv_w = [120.0f32, 196.0, 200.0, 270.0]; // 786
    let priv_cells: [(&str, &str, &str, &str); 3] = [
        (
            "xAI — Colossus (Memphis, TN)",
            "~$18B chips reported for Colossus 2 (~555k Nvidia GB200/GB300); Colossus 1 ~230k H100/H200/GB200",
            "Not separately costed: retrofit of a former Electrolux factory, ~35 on-site gas turbines, Tesla Megapacks, ~1 GW gas plant in Mississippi. Colossus 1 plant ~$6\u{2013}7B class.",
            "~70%+ silicon (inferred — only the chip order is reported; facility cost undisclosed). Sources: SiliconANGLE, Introl, Global Data Center Hub.",
        ),
        (
            "OpenAI — Stargate (Abilene, TX)",
            "~$25\u{2013}30B+ for ~400\u{2013}450k Nvidia GB200 (estimate; chips owned/financed on the Stargate side)",
            "~$15B facility for 1.2 GW — built by Crusoe, financed separately via Blue Owl/JPMorgan, explicitly excludes the chips.",
            "~65\u{2013}70% silicon — the only one of the three with a separately-financed plant figure to check against. Sources: CNBC, DataCenterDynamics, Crusoe.",
        ),
        (
            "Anthropic — mostly rented compute",
            "Largely leased, not owned: AWS Trainium2 (>1M, \"Project Rainier\"), Google TPU v7 (\u{2264}1M), Azure (Nvidia), xAI Colossus 1 (~$1.25B/mo)",
            "$50B Fluidstack build (Abernathy, TX + Lake Mariner, NY); compute-vs-real-estate split not disclosed.",
            "n/a — Anthropic's spend is dominated by multi-year compute leases (opex), not an owned chip-vs-plant capex split. Sources: Anthropic, CNBC, DataCenterDynamics.",
        ),
    ];
    let priv_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = priv_cells
        .iter()
        .map(|(co, gpu, plant, basis)| {
            vec![
                (co.to_string(), true, company_c.clone(), None),
                (gpu.to_string(), false, capex_c.clone(), None),
                (plant.to_string(), false, ink.clone(), None),
                (basis.to_string(), false, note_c.clone(), None),
            ]
        })
        .collect();
    let priv_bottom = pdf.draw_table(s_x, t4 + 6.0, &priv_w, &priv_head, &priv_rows, &pal, cell, head, lh, pad_x, pad_y);

    let pfw = PAGE_W - 2.0 * MARGIN_X;
    let mut pf = priv_bottom + 16.0;
    pf = pdf.paragraph(MARGIN_X, pf, "Industry rule of thumb for an all-in AI training cluster: GPUs/servers \u{2248} 60\u{2013}80% of capex, the physical facility (shell, power, cooling, land) \u{2248} 20\u{2013}40% (Epoch AI; SemiAnalysis). xAI's reported chip-only figure and OpenAI's separately-financed ~$15B for 1.2 GW of plant are both consistent with this.", 7.5, gray.clone(), pfw) + 3.0;
    pf = pdf.paragraph(MARGIN_X, pf, "This contrasts with the public companies' audited PP&E split on page 3: there the \"compute\" and real-estate buckets are reported line items; here both sides are estimates, and for Anthropic the spend is mostly multi-year compute leases (opex) rather than owned capital.", 7.5, gray.clone(), pfw) + 3.0;
    pf = pdf.paragraph(MARGIN_X, pf, "One exception to \"file nothing\": xAI's compute is sold through SpaceX, which is now going public. SpaceX's IPO Free Writing Prospectus (Rule 433, File 333-296070, filed 5 Jun 2026) discloses a Cloud Service Agreement with Google \u{2014} $920M/mo for ~110k Nvidia GPUs + CPUs/memory, Oct 2026\u{2013}Jun 2029 (~$11B/yr, ~$30B over the term), ramping at a reduced fee through Sep 2026. Together with the Anthropic lease ($1.25B/mo for Colossus 1), this is the first xAI/SpaceX compute revenue to surface in an SEC filing \u{2014} the rest of this page remains press/analyst estimate.", 7.5, gray.clone(), pfw) + 3.0;
    pdf.paragraph(MARGIN_X, pf, "Links to each operator's primary announcement are in the Capex column on page 1.", 7.5, gray.clone(), pfw);

    let page4 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ===================== PAGE 5: off-grid vs on-grid CAPACITY =====================
    let offc = costgw_c.clone(); // amber — off-grid (press, not in SEC)
    let secc = capex_c.clone();  // green — capacity actually disclosed in the filing

    let mut t5 = 24.0f32;
    pdf.line(MARGIN_X, by(t5), "Off-grid vs on-grid capacity — and how little of it is in the SEC filings", true, 15.0, title_c.clone(), None);
    t5 += 15.0;
    t5 = pdf.paragraph(MARGIN_X, t5, "How each operator's data centers are powered, in GW. The off-grid / behind-the-meter column is press / permit / satellite-sourced (Cleanview) — it does NOT appear in any SEC filing. On-grid capacity is an SEC figure ONLY for CoreWeave (10-K) and Nebius (20-F), shown in green; every other GW here is a press/analyst estimate, since the hyperscalers disclose no capacity and xAI/OpenAI/Anthropic file nothing.", 9.0, gray.clone(), PAGE_W - 2.0 * MARGIN_X) + 2.0;

    let og_head = [
        "Operator",
        "Off-grid / behind-the-meter capacity  (press \u{2014} NOT in SEC filings)",
        "On-grid capacity  (green = disclosed in the SEC filing)",
    ];
    let og_w = [120.0f32, 340.0, 326.0]; // 786
    // (operator, off-grid text, on-grid text, on-grid figure is SEC-disclosed)
    let og: [(&str, &str, &str, bool); 10] = [
        ("xAI \u{2014} Colossus (Memphis, TN)",
         "~1.5 GW of on-site gas generators (~60 turbines, 2 sites); ~0.3 GW live \u{2192} >1 GW (Colossus 2)",
         "\u{2014}  off-grid by design (negligible grid)", false),
        ("OpenAI \u{2014} Stargate (Abilene, TX +)",
         "Abilene: Crusoe-built on-site gas (within 1.2 GW). One planned site = 2.45 GW on-site (blocked by a New Mexico pipeline denial)",
         "\u{2014}  campus self-powered; grid share n/d", false),
        ("Meta",
         "~0.4 GW behind-the-meter gas (Williams 2\u{00d7}200 MW, New Albany OH) + a Tennessee \"tent\" site (n/d). Absent from the 10-K",
         "Several GW older fleet, grid-connected \u{2014} not quantified in SEC", false),
        ("Oracle (OCI)",
         "Operates the Crusoe-built Abilene campus (on-site gas); attribution shared with Crusoe / OpenAI",
         "~2\u{2013}3 GW OCI fleet, grid \u{2014} not quantified in SEC", false),
        ("Amazon (AWS)",
         "None identified \u{2014} grid-connected (20-yr electricity-supply contracts)",
         "~10\u{2013}15+ GW est., grid \u{2014} not quantified in SEC", false),
        ("Microsoft (Azure)",
         "None identified \u{2014} grid-connected",
         "~5\u{2013}8+ GW est., grid \u{2014} not quantified in SEC", false),
        ("Alphabet (Google)",
         "None identified \u{2014} third-party PPAs / renewables, grid",
         "Several GW est., grid \u{2014} not quantified in SEC", false),
        ("CoreWeave (CRWV)",
         "None \u{2014} on-site generation is diesel backup only",
         "0.85 GW active / 3.1 GW contracted \u{2014} disclosed in the 10-K", true),
        ("Nebius (NBIS)",
         "None \u{2014} owned / greenfield, grid-connected",
         "0.17 GW active / >2 GW contracted \u{2014} disclosed in the 20-F", true),
        ("Anthropic",
         "None owned \u{2014} rents partner compute (some itself off-grid, e.g. xAI Colossus)",
         "Via partners (AWS / Google / Azure) \u{2014} n/d", false),
    ];
    let og_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = og
        .iter()
        .map(|(op, off, on, is_sec)| {
            vec![
                (op.to_string(), true, company_c.clone(), None),
                (off.to_string(), false, offc.clone(), None),
                (on.to_string(), *is_sec, if *is_sec { secc.clone() } else { ink.clone() }, None),
            ]
        })
        .collect();
    let og_bottom = pdf.draw_table(s_x, t5 + 6.0, &og_w, &og_head, &og_rows, &pal, cell, head, lh, pad_x, pad_y);

    let ofw = PAGE_W - 2.0 * MARGIN_X;
    let mut of = og_bottom + 13.0;
    of = pdf.paragraph(MARGIN_X, of, "The capacity split (press, not SEC): Cleanview counts ~56 GW of planned behind-the-meter / off-grid capacity \u{2014} about 30% of all planned US data-center capacity \u{2014} with ~2 GW online today, almost entirely gas-fired. The other ~70% is grid-connected. None of this off-grid split appears in any SEC filing.", 7.4, offc.clone(), ofw) + 2.5;
    of = pdf.paragraph(MARGIN_X, of, "Per the filings: 0 of 7 SEC filers disclose any off-grid / self-generated capacity or an on-/off-grid split; only CoreWeave (10-K) and Nebius (20-F) quantify capacity at all \u{2014} both on-grid (active / contracted MW\u{2013}GW, in green). The hyperscalers disclose no GW; xAI, OpenAI and Anthropic file nothing.", 7.4, gray.clone(), ofw) + 2.5;
    of = pdf.paragraph(MARGIN_X, of, "Off-grid figures are press / satellite / permit-sourced: xAI ~1.5 GW of on-site turbines (Memphis); OpenAI Stargate one planned site 2.45 GW on-site (New Mexico pipeline denied); Meta's $1.6B / 400 MW Williams plant (New Albany). On-grid GW for the hyperscalers are analyst estimates, not SEC figures \u{2014} SEC filings do not disclose data-center capacity in GW.", 7.4, gray.clone(), ofw);
    pdf.line(MARGIN_X, by(of), "Cleanview \u{2014} \"Bypassing the Grid\" (behind-the-meter data centers, 2026) \u{2197}", false, 7.4, link_c.clone(), Some(CLEANVIEW));

    let page5 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ---- Save ----
    doc.with_pages(vec![page1, page2, page3, page4, page5]);
    let mut sw: Vec<printpdf::PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut sw);
    std::fs::write("pdf/datacenter_sources.pdf", &bytes).expect("write pdf");
    println!("Wrote pdf/datacenter_sources.pdf ({} bytes)", bytes.len());
}
