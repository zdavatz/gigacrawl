//! Indikationscode — Implementierungs-Leitfaden für Softwarehäuser, die
//! oddb2xml bereits als Datenquelle einsetzen. Erklärt die zwei neuen XML-
//! Elemente (`<INDIKATIONSCODE>` / `<INDIKATIONSCODE_TEXT>`), das empfohlene
//! UI-Pattern, ICD-Brücke und den Datenfluss. Zwei Seiten A4 Querformat.

use ab_glyph::{Font as AbFont, FontRef, PxScale, ScaleFont};
use printpdf::{
    Actions, BorderArray, Color, ColorArray, Line, LinePoint, LinkAnnotation, Op, PaintMode,
    PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rect, Rgb,
    TextItem, WindingOrder,
};

const FONT_REGULAR: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf");
const FONT_MONO: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSansMono.ttf");

const MM: f32 = 2.834_645;
const PAGE_W: f32 = 297.0 * MM; // A4 landscape
const PAGE_H: f32 = 210.0 * MM;
const MARGIN_X: f32 = 32.0;
const WRAP_FUDGE: f32 = 1.14;

const ODDB2XML_GITHUB: &str = "https://github.com/zdavatz/oddb2xml";
const EPL_BAG: &str = "https://epl.bag.admin.ch";
const GENERIKA_IOS: &str = "https://apps.apple.com/ch/app/generika/id520038123";

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, None))
}

struct Pdf<'a> {
    ops: Vec<Op>,
    reg: PdfFontHandle,
    bold: PdfFontHandle,
    mono: PdfFontHandle,
    reg_ab: FontRef<'a>,
    bold_ab: FontRef<'a>,
    mono_ab: FontRef<'a>,
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
                let cand = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
                if self.width(&cand, bold, size) * WRAP_FUDGE <= max_w || line.is_empty() {
                    line = cand;
                } else {
                    out.push(line);
                    line = word.to_string();
                }
            }
            out.push(line);
        }
        if out.is_empty() { out.push(String::new()); }
        out
    }

    fn line(&mut self, x: f32, by: f32, text: &str, bold: bool, size: f32, col: Color, url: Option<&str>) {
        self.line_font(x, by, text, if bold { Family::Bold } else { Family::Regular }, size, col, url);
    }

    fn line_font(&mut self, x: f32, by: f32, text: &str, fam: Family, size: f32, col: Color, url: Option<&str>) {
        let handle = match fam {
            Family::Regular => self.reg.clone(),
            Family::Bold => self.bold.clone(),
            Family::Mono => self.mono.clone(),
        };
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetFont { font: handle, size: Pt(size) });
        self.ops.push(Op::SetFillColor { col: col.clone() });
        self.ops.push(Op::SetTextCursor { pos: Point { x: Pt(x), y: Pt(by) } });
        self.ops.push(Op::ShowText { items: vec![TextItem::Text(text.to_string())] });
        self.ops.push(Op::EndTextSection);

        if let Some(url) = url {
            let bold = matches!(fam, Family::Bold);
            let w = self.width(text, bold, size) * WRAP_FUDGE;
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
                    Rect { x: Pt(x), y: Pt(by - 2.5), width: Pt(w), height: Pt(size + 2.5), mode: None, winding_order: None },
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

    fn mono_block(&mut self, x: f32, top: f32, lines: &[&str], size: f32, col: Color, bg: Color, border_c: Color, max_w: f32) -> f32 {
        let by = |t: f32| PAGE_H - t;
        let lh = size + 2.8;
        let pad = 5.0;
        let block_h = lines.len() as f32 * lh + pad * 2.0;
        // bg
        self.fill_rect(x, by(top + block_h), max_w, block_h, bg);
        // outline
        self.seg(x, by(top), x + max_w, by(top), 0.4, border_c.clone());
        self.seg(x, by(top + block_h), x + max_w, by(top + block_h), 0.4, border_c.clone());
        self.seg(x, by(top), x, by(top + block_h), 0.4, border_c.clone());
        self.seg(x + max_w, by(top), x + max_w, by(top + block_h), 0.4, border_c.clone());
        let mut y = top + pad + size;
        for ln in lines {
            self.line_font(x + pad, by(y), ln, Family::Mono, size, col.clone(), None);
            y += lh;
        }
        top + block_h
    }
}

#[derive(Copy, Clone)]
enum Family { Regular, Bold, Mono }

fn main() {
    let mut warns = Vec::new();
    let reg = printpdf::ParsedFont::from_bytes(FONT_REGULAR, 0, &mut warns).expect("reg");
    let bold = printpdf::ParsedFont::from_bytes(FONT_BOLD, 0, &mut warns).expect("bold");
    let mono = printpdf::ParsedFont::from_bytes(FONT_MONO, 0, &mut warns).expect("mono");
    let mut doc = PdfDocument::new("Indikationscode — Implementierungs-Leitfaden für Softwarehäuser (oddb2xml)");
    let reg_id = doc.add_font(&reg);
    let bold_id = doc.add_font(&bold);
    let mono_id = doc.add_font(&mono);
    let mut pdf = Pdf {
        ops: Vec::new(),
        reg: PdfFontHandle::External(reg_id),
        bold: PdfFontHandle::External(bold_id),
        mono: PdfFontHandle::External(mono_id),
        reg_ab: FontRef::try_from_slice(FONT_REGULAR).unwrap(),
        bold_ab: FontRef::try_from_slice(FONT_BOLD).unwrap(),
        mono_ab: FontRef::try_from_slice(FONT_MONO).unwrap(),
    };

    let title_c = rgb(15, 23, 42);
    let gray = rgb(100, 116, 139);
    let ink = rgb(30, 41, 59);
    let accent = rgb(13, 71, 161);
    let highlight = rgb(21, 101, 52);
    let warn = rgb(180, 83, 9);
    let code_bg = rgb(244, 246, 250);
    let code_border = rgb(203, 213, 225);
    let code_fg = rgb(40, 50, 70);
    let link_c = rgb(13, 71, 161);

    let by = |t: f32| PAGE_H - t;
    let fw = PAGE_W - 2.0 * MARGIN_X;

    // ============================================================
    // PAGE 1 — Datenquelle + UI-Pattern
    // ============================================================
    let mut top = 26.0f32;
    pdf.line(MARGIN_X, by(top),
        "Indikationscode (IndC) — Integrations-Leitfaden für Softwarehäuser",
        true, 16.0, title_c.clone(), None);
    top += 18.0;
    pdf.line(MARGIN_X, by(top),
        "Stichtag 01.07.2026: IndC ist Pflichtangabe auf jedem SL-Rezept und jeder SL-Rechnung. Ab 01.01.2027: Rückweisungsgrund. \
         Wer oddb2xml bereits als Datenquelle einsetzt, kommt mit minimalem Aufwand ans Ziel.",
        false, 9.5, gray.clone(), None);
    top += 22.0;

    // Two-column layout: left = Datenquelle, right = UI-Pattern
    let col_gap = 18.0f32;
    let left_w = (fw * 0.52).round();
    let right_w = fw - col_gap - left_w;
    let left_x = MARGIN_X;
    let right_x = MARGIN_X + left_w + col_gap;

    // === LEFT COLUMN: Datenquelle ===
    pdf.line(left_x, by(top), "1.  Datenquelle: oddb2xml liefert den IndC bereits mit",
        true, 11.5, accent.clone(), None);
    let mut ly = top + 18.0;
    ly = pdf.paragraph(left_x, ly,
        "Seit oddb2xml Version 3.1.12 sind zwei zusätzliche XML-Elemente pro Artikel im Stream — gezogen direkt aus dem BAG SL-FHIR-Feed (epl.bag.admin.ch), tagesaktuell:",
        9.0, ink.clone(), left_w) + 4.0;

    let xml_lines = [
        "<ARTICLE>",
        "  <GTIN>7680543780176</GTIN>",
        "  <DSCR>MabThera Inf Konz 100 mg/10ml</DSCR>",
        "  <ATC>L01FA01</ATC>",
        "  <INDIKATIONSCODE>17079.01,17079.02,17079.04</INDIKATIONSCODE>",
        "  <INDIKATIONSCODE_TEXT>17079.01: Hämatologie — CD20+ NHL...",
        "17079.02: Autoimmun — rheumatoide Arthritis ...",
        "17079.04: Kombi MabThera + Polivy + Bendamustin ...</INDIKATIONSCODE_TEXT>",
        "</ARTICLE>",
    ];
    ly = pdf.mono_block(left_x, ly, &xml_lines, 7.5, code_fg.clone(), code_bg.clone(), code_border.clone(), left_w) + 8.0;

    pdf.line(left_x, by(ly), "•", true, 9.5, accent.clone(), None);
    ly = pdf.paragraph(left_x + 10.0, ly,
        "INDIKATIONSCODE: kommaseparierte Liste der XXXXX.NN zum jeweiligen GTIN bzw. BAG-Dossier. Reihenfolge nach BAG-Bundle.",
        9.0, ink.clone(), left_w - 10.0) + 3.0;
    pdf.line(left_x, by(ly), "•", true, 9.5, accent.clone(), None);
    ly = pdf.paragraph(left_x + 10.0, ly,
        "INDIKATIONSCODE_TEXT: zeilenweise XXXXX.NN: <Klartext-Limitation aus der SL>. Reihenfolge & Codes identisch mit INDIKATIONSCODE.",
        9.0, ink.clone(), left_w - 10.0) + 3.0;
    pdf.line(left_x, by(ly), "•", true, 9.5, accent.clone(), None);
    ly = pdf.paragraph(left_x + 10.0, ly,
        "Falls Ihre aktuelle oddb2xml-Version älter ist und die Elemente fehlen: nur updaten — kein Schema-Wechsel, beide neuen Elemente kommen am Tail dazu.",
        9.0, ink.clone(), left_w - 10.0) + 3.0;
    pdf.line(left_x, by(ly), "•", true, 9.5, accent.clone(), None);
    ly = pdf.paragraph(left_x + 10.0, ly,
        "Bei Artikeln ohne Preismodell: beide Elemente sind leer (<INDIKATIONSCODE/>) — UI muss kein Dropdown anzeigen.",
        9.0, ink.clone(), left_w - 10.0);

    // === RIGHT COLUMN: UI-Pattern ===
    pdf.line(right_x, by(top), "2.  UI-Pattern: ein Klick (oder gar nichts)",
        true, 11.5, accent.clone(), None);
    let mut ry = top + 18.0;
    ry = pdf.paragraph(right_x, ry,
        "Adminimierung in der Praxis: das System trifft die Auswahl, wo eindeutig — sonst fragt es einmal.",
        9.0, ink.clone(), right_w) + 6.0;

    // Decision tree
    pdf.line(right_x, by(ry), "Beim Verordnen / bei Rezeptaufnahme:",
        true, 9.5, title_c.clone(), None);
    ry += 14.0;

    let cases = [
        ("0 Codes",  "Kein Preismodell — IndC nicht erforderlich, kein UI-Schritt.",  highlight.clone()),
        ("1 Code",   "Auto-selected; nur Info-Hinweis im Rezept-Footer.",              highlight.clone()),
        (">1 Codes", "Kompakte Dropdown «XXXXX.NN — Klartext» pro Artikel. Ein Klick.", warn.clone()),
    ];
    for (lbl, descr, c) in &cases {
        pdf.fill_rect(right_x, by(ry + 2.0), 3.5, 14.0, c.clone());
        pdf.line(right_x + 9.0, by(ry + 9.5), lbl, true, 9.5, c.clone(), None);
        let lbl_w = pdf.width(lbl, true, 9.5) * WRAP_FUDGE;
        let body_x = right_x + 9.0 + lbl_w + 8.0;
        ry = pdf.paragraph(body_x, ry + 9.5, descr, 9.0, ink.clone(), right_w - (body_x - right_x)) + 6.0;
    }
    ry += 4.0;

    pdf.line(right_x, by(ry), "Zusatz-Mechanik (alle optional, alle möglich ohne Mehraufwand):",
        true, 9.5, title_c.clone(), None);
    ry += 14.0;

    let extras = [
        ("Persistenz",  "Einmal gesetzter IndC für eine Dauertherapie wird bei Folgeverordnung vorgeschlagen — Pflegezeit 0."),
        ("ICD-Brücke",  "Wenn ICD-10 / Tessinercode in der Software dokumentiert, kann das System den wahrscheinlichsten IndC vorschlagen (nicht setzen). Mapping ICD → IndC stellen wir bereit."),
        ("Indikations-Wechsel", "Wenn der Arzt das Indikationsfeld ändert, schlägt das System den passenden IndC neu vor und fragt nur, wenn mehrere möglich."),
        ("Suchhilfe",   "Volltextsuche auf INDIKATIONSCODE_TEXT (z. B. «Mamma» → liefert 18082.04). Spart das ePL-Browsen."),
    ];
    for (k, v) in &extras {
        pdf.line(right_x, by(ry), "•", true, 9.5, accent.clone(), None);
        pdf.line(right_x + 10.0, by(ry), k, true, 9.0, title_c.clone(), None);
        let kw = pdf.width(k, true, 9.0) * WRAP_FUDGE;
        ry = pdf.paragraph(right_x + 10.0 + kw + 6.0, ry, v, 9.0, ink.clone(), right_w - (10.0 + kw + 6.0)) + 3.0;
    }

    // Footer Page 1 — sources
    let foot = ly.max(ry) + 18.0;
    pdf.line(MARGIN_X, by(foot), "Quellen:", true, 8.5, title_c.clone(), None);
    let mut lx = MARGIN_X + pdf.width("Quellen:", true, 8.5) * WRAP_FUDGE + 10.0;
    let l1 = "github.com/zdavatz/oddb2xml ↗";
    pdf.line(lx, by(foot), l1, false, 8.5, link_c.clone(), Some(ODDB2XML_GITHUB));
    lx += pdf.width(l1, false, 8.5) * WRAP_FUDGE + 14.0;
    let l2 = "BAG SL FHIR-Feed ↗";
    pdf.line(lx, by(foot), l2, false, 8.5, link_c.clone(), Some(EPL_BAG));
    lx += pdf.width(l2, false, 8.5) * WRAP_FUDGE + 14.0;
    pdf.line(lx, by(foot), "Generika.cc App (Referenz-Implementierung) ↗", false, 8.5, link_c.clone(), Some(GENERIKA_IOS));

    let page1 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    // ============================================================
    // PAGE 2 — Datenfluss + Beispiel + Mapping nach aussen
    // ============================================================
    let mut top = 26.0f32;
    pdf.line(MARGIN_X, by(top),
        "Datenfluss von der SL bis zur Kasse — und was an welcher Stelle zu tun ist",
        true, 15.0, title_c.clone(), None);
    top += 16.0;
    pdf.line(MARGIN_X, by(top),
        "Wer oddb2xml bereits konsumiert, hat den Daten-Teil. Übrig bleibt eine kleine UI-Erweiterung und ein zusätzliches Feld im ausgehenden Rezept-/Rechnungs-Format.",
        false, 9.0, gray.clone(), None);
    top += 18.0;

    // Pipeline strip — coloured boxes
    let boxes: [(&str, &str, Color); 5] = [
        ("BAG SL FHIR",     "epl.bag.admin.ch\n(NDJSON, täglich)", rgb(30, 58, 95)),
        ("oddb2xml",        "<INDIKATIONSCODE>\n<INDIKATIONSCODE_TEXT>", rgb(13, 71, 161)),
        ("Praxis-Software", "GTIN-Lookup, UI-Dropdown,\nPersistenz pro Patient",  rgb(21, 101, 52)),
        ("KVV-71 / Rezept", "IndC eingebettet in PDF\nund CHMED16A QR",  rgb(146, 64, 14)),
        ("Krankenversicherer", "validiert IndC,\nrechnet Rückerstattung ab", rgb(120, 53, 15)),
    ];
    let bx_gap = 8.0f32;
    let bx_w = (fw - bx_gap * (boxes.len() as f32 - 1.0)) / boxes.len() as f32;
    let bx_h = 46.0f32;
    let mut bx = MARGIN_X;
    for (i, (label, body, c)) in boxes.iter().enumerate() {
        pdf.fill_rect(bx, by(top + bx_h), bx_w, bx_h, c.clone());
        pdf.line(bx + 6.0, by(top + 12.0), label, true, 9.5, rgb(255, 255, 255), None);
        let body_lines = pdf.wrap(body, false, 8.0, bx_w - 12.0);
        let mut bly = top + 22.0;
        for ln in &body_lines {
            pdf.line(bx + 6.0, by(bly), ln, false, 8.0, rgb(232, 240, 254), None);
            bly += 10.0;
        }
        // arrow
        if i < boxes.len() - 1 {
            let ax = bx + bx_w + 1.0;
            let ay = by(top + bx_h * 0.5);
            pdf.seg(ax, ay, ax + bx_gap - 2.0, ay, 1.2, rgb(100, 116, 139));
            // arrowhead
            pdf.seg(ax + bx_gap - 4.0, ay + 2.5, ax + bx_gap - 2.0, ay, 1.2, rgb(100, 116, 139));
            pdf.seg(ax + bx_gap - 4.0, ay - 2.5, ax + bx_gap - 2.0, ay, 1.2, rgb(100, 116, 139));
        }
        bx += bx_w + bx_gap;
    }
    top += bx_h + 18.0;

    // Two columns again
    let left_w = (fw * 0.48).round();
    let right_w = fw - col_gap - left_w;
    let left_x = MARGIN_X;
    let right_x = MARGIN_X + left_w + col_gap;

    // LEFT: outbound mapping
    pdf.line(left_x, by(top),
        "Datenfluss raus: was am Ausgang stehen muss",
        true, 11.0, accent.clone(), None);
    let mut ly = top + 16.0;
    ly = pdf.paragraph(left_x, ly,
        "Empfohlenes JSON-Modell, das Praxis-Software an das KVV-71-PDF und ans Rezept anhängt:",
        9.0, ink.clone(), left_w) + 4.0;

    let json_lines = [
        "{",
        "  \"gtin\":  \"7680543780176\",",
        "  \"brand\": \"MabThera Inf Konz 100 mg/10ml\",",
        "  \"indc\":  \"17079.04\",",
        "  \"indc_text\": \"Kombi MabThera + Polivy + ...\",",
        "  \"icd10\": \"C83.3\",",
        "  \"selected_by\": \"user\"   // oder \"auto\" / \"persisted\"",
        "}",
    ];
    ly = pdf.mono_block(left_x, ly, &json_lines, 7.8, code_fg.clone(), code_bg.clone(), code_border.clone(), left_w) + 8.0;

    ly = pdf.paragraph(left_x, ly,
        "Für CHMED16A QR-Codes: IndC im Custom-Feld neben dem GTIN mittragen. Die Annahme­software (z. B. bei Zur Rose) kann dann ohne Klartext-Lookup direkt weiter verarbeiten — die Krankenkasse erhält den Code als Teil der Rechnungsdatensätze.",
        9.0, ink.clone(), left_w) + 4.0;

    pdf.line(left_x, by(ly), "Validierung an der Abrechnungsseite:",
        true, 9.5, title_c.clone(), None);
    ly += 14.0;
    let val_items = [
        "Code-Format-Check: ^[0-9]{5}\\.[0-9]{2}$",
        "Existenz-Check: IndC ist im aktuellen oddb2xml-Stream für diesen GTIN gelistet",
        "Konsistenz-Check: IndC und GTIN gehören zur selben BAG-Dossiernummer",
        "Aktualitäts-Check: Datum der oddb2xml-Generierung im SOAP-Header oder als XML-Attribut mitliefern",
    ];
    for v in &val_items {
        pdf.line(left_x, by(ly), "•", true, 9.0, accent.clone(), None);
        ly = pdf.paragraph(left_x + 10.0, ly, v, 9.0, ink.clone(), left_w - 10.0) + 1.0;
    }

    // RIGHT: code samples & numbers
    pdf.line(right_x, by(top),
        "Eckwerte aus dem aktuellen BAG SL FHIR-Feed",
        true, 11.0, accent.clone(), None);
    let mut ry = top + 16.0;
    let kpi_strip: [(&str, &str, Color); 4] = [
        ("1'419", "IndC-Zeilen",          accent.clone()),
        ("571",   "distinkte Codes",      accent.clone()),
        ("264",   "BAG-Dossiers",         highlight.clone()),
        ("77 %",  "ATC L (Onko/Immun)",   warn.clone()),
    ];
    let kgap = 8.0f32;
    let kw = (right_w - kgap * 3.0) / 4.0;
    let kh = 38.0f32;
    for (i, (v, l, c)) in kpi_strip.iter().enumerate() {
        let kx = right_x + i as f32 * (kw + kgap);
        pdf.fill_rect(kx, by(ry + kh), 2.5, kh, c.clone());
        pdf.seg(kx, by(ry), kx + kw, by(ry), 0.4, code_border.clone());
        pdf.seg(kx, by(ry + kh), kx + kw, by(ry + kh), 0.4, code_border.clone());
        pdf.seg(kx + kw, by(ry), kx + kw, by(ry + kh), 0.4, code_border.clone());
        pdf.line(kx + 6.0, by(ry + 16.0), v, true, 13.0, c.clone(), None);
        pdf.line(kx + 6.0, by(ry + 30.0), l, false, 8.0, gray.clone(), None);
    }
    ry += kh + 14.0;

    pdf.line(right_x, by(ry),
        "Was Verschreibende auf einem typischen GTIN-Lookup sehen",
        true, 10.5, title_c.clone(), None);
    ry += 14.0;
    ry = pdf.paragraph(right_x, ry,
        "Beispiel-Antwort der Mapping-API (oddb2xml-Stream → JSON-Endpoint) für MabThera:",
        9.0, ink.clone(), right_w) + 4.0;
    let api_lines = [
        "GET /indc?gtin=7680543780176",
        "",
        "{",
        "  \"gtin\":  \"7680543780176\",",
        "  \"brand\": \"MabThera Inf Konz 100 mg/10ml\",",
        "  \"bag_dossier\": \"17079\",",
        "  \"options\": [",
        "    {\"code\": \"17079.01\", \"text\": \"CD20+ NHL ...\"},",
        "    {\"code\": \"17079.02\", \"text\": \"Autoimmun (RA) ...\"},",
        "    {\"code\": \"17079.04\", \"text\": \"Kombi mit Polivy ...\"}",
        "  ]",
        "}",
    ];
    ry = pdf.mono_block(right_x, ry, &api_lines, 7.8, code_fg.clone(), code_bg.clone(), code_border.clone(), right_w) + 8.0;

    ry = pdf.paragraph(right_x, ry,
        "Die Generika.cc App ist die Referenz-Implementierung dieses Patterns (KVV 71 mit IndC-Auswähler, embedded in PDF und E-Mail-Versand) und kann gleichzeitig als End-to-End-Testfall dienen.",
        9.0, ink.clone(), right_w);

    // Footer
    let foot = ly.max(ry) + 16.0;
    pdf.paragraph(MARGIN_X, foot,
        "Bei Fragen zur Integration, Mapping-Datei (CSV/JSON/SQLite-Snapshot), API-Endpoint oder gemeinsamem Test mit einer Praxis-Software: einfach melden. Open Source — oddb2xml und die Generika.cc App sind auf GitHub und auf den Stores verfügbar.",
        8.5, gray.clone(), fw);

    let page2 = PdfPage::new(printpdf::Mm(297.0), printpdf::Mm(210.0), std::mem::take(&mut pdf.ops));

    doc.with_pages(vec![page1, page2]);
    let mut sw: Vec<printpdf::PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut sw);
    std::fs::create_dir_all("pdf").ok();
    std::fs::write("pdf/indc_integration.pdf", &bytes).expect("write pdf");
    println!("Wrote pdf/indc_integration.pdf ({} bytes)", bytes.len());
}
