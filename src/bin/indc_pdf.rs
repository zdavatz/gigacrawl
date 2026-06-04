//! Indikationscode (IndC) — companion PDF for the PNG overview.
//!
//! Two-page landscape A4: page 1 = headline KPIs + ATC main-class
//! distribution + key dates from the BAG Rundschreiben; page 2 = sample IndC
//! entries from the BAG SL FHIR feed + the top brands by # of distinct codes.
//! Outbound link goes to the BAG Rundschreiben on bag.admin.ch.

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
const MARGIN_X: f32 = 32.0;
const WRAP_FUDGE: f32 = 1.14;

// Sources
// All three URLs verified live (HTTP 200) when this file was last edited.
// The direct PDF deeplink to the 19.02.2026 Rundschreiben on bag.admin.ch
// could not be verified, so the link points at the SL portal where the
// Rundschreiben is published.
const BAG_RUNDSCHREIBEN: &str = "https://www.spezialitaetenliste.ch/";
const EPL_BAG: &str = "https://epl.bag.admin.ch";
const CPP2SQLITE: &str = "https://github.com/zdavatz/cpp2sqlite";
const INDC_XLSX: &str = "https://github.com/zdavatz/gigacrawl/blob/main/xlsx/indc.xlsx";

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

    /// Self-contained table renderer (header band + alternating rows + grid).
    /// Each cell is (text, bold, color, optional link URL).
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

struct Pal {
    header_bg: Color,
    header_fg: Color,
    row_a: Color,
    row_b: Color,
    border: Color,
    outer: Color,
}

fn main() {
    let mut warns = Vec::new();
    let reg = printpdf::ParsedFont::from_bytes(FONT_REGULAR, 0, &mut warns).expect("reg");
    let bold = printpdf::ParsedFont::from_bytes(FONT_BOLD, 0, &mut warns).expect("bold");
    let mut doc = PdfDocument::new("Indikationscode (IndC) — SL-Pflichtangabe ab 01.07.2026");
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
    let title_c = rgb(15, 23, 42);
    let gray = rgb(100, 116, 139);
    let accent = rgb(13, 71, 161);
    let highlight = rgb(21, 101, 52);
    let warn = rgb(180, 83, 9);
    let border = rgb(203, 213, 225);
    let outer = rgb(148, 163, 184);
    let link_c = rgb(13, 71, 161);

    let pal = Pal {
        header_bg: header_bg.clone(),
        header_fg: header_fg.clone(),
        row_a: row_a.clone(),
        row_b: row_b.clone(),
        border: border.clone(),
        outer: outer.clone(),
    };

    let by = |t: f32| PAGE_H - t;
    let fw = PAGE_W - 2.0 * MARGIN_X;

    // ============================================================
    // PAGE 1 — Overview
    // ============================================================
    let mut top = 26.0f32;
    pdf.line(MARGIN_X, by(top),
        "Indikationscode (IndC) — die SL-Pflichtangabe ab 01.07.2026",
        true, 17.0, title_c.clone(), None);
    top += 18.0;
    pdf.line(MARGIN_X, by(top),
        "Daten aus dem BAG SL FHIR-Feed (epl.bag.admin.ch), ausgelesen mit cpp2sqlite. \
         Format: XXXXX.NN — fünfstellige BAG-Dossiernummer + zweistellige Indikations-Nummer.",
        false, 9.0, gray.clone(), None);
    top += 18.0;

    // KPI strip — four boxes
    let kpi_gap = 10.0f32;
    let kpi_w = (fw - kpi_gap * 3.0) / 4.0;
    let kpi_h = 56.0f32;
    let kpis: [(&str, &str, Color); 4] = [
        ("1'419", "IndC-Zeilen im SL-Feed", accent.clone()),
        ("571",   "unterschiedliche XXXXX.NN-Codes", accent.clone()),
        ("264",   "BAG-Dossiernummern", highlight.clone()),
        ("77 %",  "Anteil ATC L (Onkologie/Immun)", warn.clone()),
    ];
    for (i, (v, l, c)) in kpis.iter().enumerate() {
        let x = MARGIN_X + i as f32 * (kpi_w + kpi_gap);
        pdf.fill_rect(x, by(top + kpi_h), kpi_w, kpi_h, row_a.clone());
        // accent stripe
        pdf.fill_rect(x, by(top + kpi_h), 3.0, kpi_h, c.clone());
        // outline
        pdf.seg(x, by(top), x + kpi_w, by(top), 0.5, border.clone());
        pdf.seg(x, by(top + kpi_h), x + kpi_w, by(top + kpi_h), 0.5, border.clone());
        pdf.seg(x + kpi_w, by(top), x + kpi_w, by(top + kpi_h), 0.5, border.clone());
        // value
        pdf.line(x + 8.0, by(top + 24.0), v, true, 20.0, c.clone(), None);
        // label (wrap)
        let lbl_lines = pdf.wrap(l, false, 8.5, kpi_w - 14.0);
        let mut ly = top + 32.0;
        for ln in &lbl_lines {
            pdf.line(x + 8.0, by(ly + 8.5), ln, false, 8.5, gray.clone(), None);
            ly += 10.0;
        }
    }
    top += kpi_h + 20.0;

    // Two-column section: left = ATC table, right = key dates + context
    let col_gap = 14.0f32;
    let left_w = (fw * 0.62).round();
    let right_w = fw - col_gap - left_w;
    let left_x = MARGIN_X;
    let right_x = MARGIN_X + left_w + col_gap;

    // LEFT: ATC table
    pdf.line(left_x, by(top),
        "Verteilung der Indikationscodes nach ATC-Hauptklasse (n = 1'419)",
        true, 11.0, title_c.clone(), None);
    let atc_data: [(&str, &str, &str, &str); 13] = [
        ("L", "Antineoplastika & Immunmodulatoren", "1'094", "77.1 %"),
        ("B", "Blut & blutbildende Organe",          "123",   "8.7 %"),
        ("N", "Nervensystem",                         "47",   "3.3 %"),
        ("A", "Alimentäres System & Stoffwechsel",   "34",   "2.4 %"),
        ("C", "Kardiovaskuläres System",              "30",   "2.1 %"),
        ("D", "Dermatologika",                        "29",   "2.0 %"),
        ("J", "Antiinfektiva (systemisch)",           "21",   "1.5 %"),
        ("R", "Atmungssystem",                        "11",   "0.8 %"),
        ("S", "Sinnesorgane",                         "10",   "0.7 %"),
        ("M", "Muskel-/Skelettsystem",                "10",   "0.7 %"),
        ("H", "Hormone (systemisch)",                  "5",   "0.4 %"),
        ("V", "Varia",                                 "3",   "0.2 %"),
        ("G", "Urogenital & Sexualhormone",            "2",   "0.1 %"),
    ];
    let atc_headers = ["ATC", "Bezeichnung (WHO ATC-Hauptklasse)", "n", "Anteil"];
    let atc_w = [32.0f32, left_w - 32.0 - 50.0 - 56.0, 50.0, 56.0];
    let atc_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = atc_data
        .iter()
        .map(|(code, name, n, pct)| {
            vec![
                (code.to_string(), true, accent.clone(), None),
                (name.to_string(), false, ink.clone(), None),
                (n.to_string(), true, ink.clone(), None),
                (pct.to_string(), false, ink.clone(), None),
            ]
        })
        .collect();
    let atc_bottom = pdf.draw_table(
        left_x, top + 16.0, &atc_w, &atc_headers, &atc_rows,
        &pal, 7.5, 7.8, 9.5, 4.0, 3.5,
    );

    // RIGHT: key dates + context. `ry` is treated as the BASELINE of the next
    // line throughout (paragraph() uses the same convention).
    pdf.line(right_x, by(top),
        "Wichtige Termine (Rundschreiben 19.02.2026)",
        true, 11.0, title_c.clone(), None);
    let mut ry = top + 16.0;
    pdf.line(right_x, by(ry), "01.07.2026", true, 9.5, warn.clone(), None);
    ry += 11.5;
    ry = pdf.paragraph(right_x, ry,
        "Übermittlung des Indikationscodes mit jeder Verordnung und jeder Rechnung für SL-Arzneimittel.",
        8.5, ink.clone(), right_w) + 4.0;
    pdf.line(right_x, by(ry), "01.01.2027", true, 9.5, warn.clone(), None);
    ry += 11.5;
    ry = pdf.paragraph(right_x, ry,
        "Krankenversicherer dürfen Rechnungen ohne IndC zurückweisen.",
        8.5, ink.clone(), right_w) + 10.0;

    pdf.line(right_x, by(ry), "Hintergrund", true, 10.0, title_c.clone(), None);
    ry += 13.0;
    ry = pdf.paragraph(right_x, ry,
        "Bei Arzneimitteln mit Preismodell wird der SL-Listenpreis vergütet; ein Teil des Fabrikabgabepreises (FAP) fliesst als Rückerstattung vom Pharmaunternehmen an den Versicherer zurück.",
        8.5, ink.clone(), right_w) + 3.0;
    ry = pdf.paragraph(right_x, ry,
        "Der IndC (Format XXXXX.NN) erlaubt die eindeutige Zuordnung Arzneimittel \u{2194} Indikation \u{2194} Rückerstattung — entscheidend für die Wirtschaftlichkeit nach KVG Art. 42 Abs. 3.",
        8.5, ink.clone(), right_w) + 3.0;
    ry = pdf.paragraph(right_x, ry,
        "Stand 01.01.2026 tragen rund 170 SL-Arzneimittel die PM-Kennzeichnung («Ja»/«Nein»).",
        8.5, ink.clone(), right_w);

    // Footer / sources for page 1. Apply WRAP_FUDGE to link-text widths since
    // ab_glyph under-measures vs. the actual rendered advance.
    let foot = atc_bottom.max(ry) + 14.0;
    let label = "Quellen:";
    pdf.line(MARGIN_X, by(foot), label, true, 8.5, title_c.clone(), None);
    let mut lx = MARGIN_X + pdf.width(label, true, 8.5) * WRAP_FUDGE + 10.0;
    let l1 = "BAG Rundschreiben vom 19.02.2026 \u{2197}";
    pdf.line(lx, by(foot), l1, false, 8.5, link_c.clone(), Some(BAG_RUNDSCHREIBEN));
    lx += pdf.width(l1, false, 8.5) * WRAP_FUDGE + 14.0;
    let l2 = "BAG SL FHIR-Feed \u{2197}";
    pdf.line(lx, by(foot), l2, false, 8.5, link_c.clone(), Some(EPL_BAG));
    lx += pdf.width(l2, false, 8.5) * WRAP_FUDGE + 14.0;
    let l3 = "Rohdaten (indc.xlsx) \u{2197}";
    pdf.line(lx, by(foot), l3, false, 8.5, link_c.clone(), Some(INDC_XLSX));
    lx += pdf.width(l3, false, 8.5) * WRAP_FUDGE + 14.0;
    pdf.line(lx, by(foot), "cpp2sqlite \u{2197}", false, 8.5, link_c.clone(), Some(CPP2SQLITE));

    let page1 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ============================================================
    // PAGE 2 — Sample IndC entries + top brands
    // ============================================================
    let mut top = 26.0f32;
    pdf.line(MARGIN_X, by(top),
        "Indikationscode (IndC) — Beispiele aus dem SL-Feed",
        true, 16.0, title_c.clone(), None);
    top += 16.0;
    pdf.line(MARGIN_X, by(top),
        "Auszug aus den 1'419 IndC-Zeilen. Pro Indikation eines Arzneimittels mit Preismodell wird ein eigener Code vergeben.",
        false, 9.0, gray.clone(), None);
    top += 14.0;

    // Sample IndC table
    let s_headers = ["IndC", "Markenname", "ATC", "EFP (CHF)", "Indikation (Auszug)"];
    let s_w = [50.0f32, 160.0, 56.0, 60.0, fw - (50.0 + 160.0 + 56.0 + 60.0)];
    let s_rows_data: &[(&str, &str, &str, &str, &str)] = &[
        ("17079.01", "MabThera Inf Konz 100 mg/10ml", "L01FA01", "518.60",
         "Hämatologie — CD20+ follikuläres Non-Hodgkin-Lymphom (Stad. III–IV), Kombination mit CVP oder CHOP; Erhaltungstherapie mit Rituximab-Monotherapie über 2 Jahre."),
        ("17079.02", "MabThera Inf Konz 100 mg/10ml", "L01FA01", "518.60",
         "Autoimmunerkrankungen — rheumatoide Arthritis und weitere im Fachinformations-Text genannte Indikationen."),
        ("17079.04", "MabThera Inf Konz 100 mg/10ml", "L01FA01", "518.60",
         "Kombination MabThera + Polivy + Bendamustin bei rezidiviertem oder refraktärem DLBCL."),
        ("18082.01", "Avastin Inf Konz 100 mg/4ml", "L01FG01", "563.40",
         "Kolorektalkarzinom."),
        ("18082.02", "Avastin Inf Konz 100 mg/4ml", "L01FG01", "563.40",
         "Lungenkarzinom."),
        ("18082.03", "Avastin Inf Konz 100 mg/4ml", "L01FG01", "563.40",
         "Nierenzellkarzinom."),
        ("18082.04", "Avastin Inf Konz 100 mg/4ml", "L01FG01", "563.40",
         "Mammakarzinom."),
        ("18082.05", "Avastin Inf Konz 100 mg/4ml", "L01FG01", "563.40",
         "Ovarialkarzinom."),
    ];
    let s_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = s_rows_data
        .iter()
        .map(|(code, name, atc, efp, indc)| {
            vec![
                (code.to_string(), true, accent.clone(), None),
                (name.to_string(), false, ink.clone(), None),
                (atc.to_string(), false, ink.clone(), None),
                (efp.to_string(), true, highlight.clone(), None),
                (indc.to_string(), false, ink.clone(), None),
            ]
        })
        .collect();
    let s_bottom = pdf.draw_table(
        MARGIN_X, top, &s_w, &s_headers, &s_rows,
        &pal, 7.8, 8.2, 10.0, 4.0, 4.0,
    );

    // Top brands
    let mut t2 = s_bottom + 18.0;
    pdf.line(MARGIN_X, by(t2),
        "Top-Präparate nach Anzahl distinkter IndC-Codes",
        true, 12.0, title_c.clone(), None);
    t2 += 14.0;
    pdf.line(MARGIN_X, by(t2),
        "Je mehr vergütete Indikationen ein Arzneimittel hat, desto mehr Codes — und desto wichtiger ist die korrekte Erfassung pro Rezept/Rechnung.",
        false, 9.0, gray.clone(), None);
    t2 += 14.0;

    let b_headers = ["#", "Markenname", "Anzahl IndC"];
    let b_w = [40.0f32, fw - 40.0 - 90.0, 90.0];
    let brands: &[(&str, &str)] = &[
        ("1",  "Keytruda Inf Konz 100 mg/4ml"),
        ("2",  "Opdivo Inf Konz (100 / 240 / 40 mg)"),
        ("3",  "Dupixent Inj Lös (200 mg/1.14 ml / 300 mg/2 ml)"),
        ("4",  "Vegzelma Inf Konz (100 / 400 mg)"),
        ("5",  "Abevmy Inf Konz (100 / 400 mg)"),
        ("6",  "Mvasi Inf Konz (100 / 400 mg)"),
        ("7",  "Bevacizumab-Teva Inf Konz (100 / 400 mg)"),
        ("8",  "Avastin Inf Konz 100 mg/4ml"),
    ];
    let b_counts: &[&str] = &["23", "14", "10", "10", "9", "8", "8", "7"];
    let b_rows: Vec<Vec<(String, bool, Color, Option<String>)>> = brands
        .iter()
        .zip(b_counts.iter())
        .map(|((rank, name), cnt)| {
            vec![
                (rank.to_string(), true, gray.clone(), None),
                (name.to_string(), false, ink.clone(), None),
                (cnt.to_string(), true, accent.clone(), None),
            ]
        })
        .collect();
    let b_bottom = pdf.draw_table(
        MARGIN_X, t2, &b_w, &b_headers, &b_rows,
        &pal, 8.0, 8.2, 10.0, 4.0, 3.8,
    );

    // Footer
    let mut foot = b_bottom + 14.0;
    foot = pdf.paragraph(MARGIN_X, foot,
        "Preisangaben: Publikumspreis aus dem BAG SL FHIR-Feed (Stand 06.2026). Die hier gezeigten Indikationstexte sind sprachlich gekürzte Auszüge; der vollständige, verbindliche Text liegt in der Spezialitätenliste (ClinicalUseDefinition).",
        7.8, gray.clone(), fw);
    foot += 2.0;
    let f1 = "Quelle Rundschreiben \u{2197}";
    pdf.line(MARGIN_X, by(foot), f1, false, 8.0, link_c.clone(), Some(BAG_RUNDSCHREIBEN));
    let mut lx = MARGIN_X + pdf.width(f1, false, 8.0) * WRAP_FUDGE + 14.0;
    let f2 = "BAG SL FHIR-Feed \u{2197}";
    pdf.line(lx, by(foot), f2, false, 8.0, link_c.clone(), Some(EPL_BAG));
    lx += pdf.width(f2, false, 8.0) * WRAP_FUDGE + 14.0;
    let f3 = "Rohdaten (indc.xlsx) \u{2197}";
    pdf.line(lx, by(foot), f3, false, 8.0, link_c.clone(), Some(INDC_XLSX));
    lx += pdf.width(f3, false, 8.0) * WRAP_FUDGE + 14.0;
    pdf.line(lx, by(foot), "Tooling: cpp2sqlite \u{2197}", false, 8.0, link_c.clone(), Some(CPP2SQLITE));

    let page2 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    doc.with_pages(vec![page1, page2]);
    let mut sw: Vec<printpdf::PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut sw);
    std::fs::create_dir_all("pdf").ok();
    std::fs::write("pdf/indc_overview.pdf", &bytes).expect("write pdf");
    println!("Wrote pdf/indc_overview.pdf ({} bytes)", bytes.len());
}
