//! Indikationscode (IndC) — Swiss SL prescription/invoice mandate.
//!
//! Renders a single PNG summarising the BAG IndC dataset extracted from the
//! BAG SL FHIR ndjson feed: headline KPIs, ATC main-class distribution,
//! sample IndC entries and the regulatory timeline (BAG Rundschreiben
//! 2026-02-19). Output: `png/indc_overview.png`.

use ab_glyph::{Font, FontRef, Glyph, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

const FONT_REGULAR: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf");

type Color = [u8; 4];

const BG: Color = [248, 250, 252, 255];
const CARD_BG: Color = [255, 255, 255, 255];
const TITLE_FG: Color = [15, 23, 42, 255];
const SUBTITLE_FG: Color = [71, 85, 105, 255];
const HEADER_BG: Color = [30, 58, 95, 255];
const HEADER_FG: Color = [255, 255, 255, 255];
const ROW_A: Color = [255, 255, 255, 255];
const ROW_B: Color = [237, 242, 248, 255];
const CELL_FG: Color = [30, 41, 59, 255];
const BORDER: Color = [203, 213, 225, 255];
const OUTER: Color = [148, 163, 184, 255];
const NOTE_FG: Color = [100, 116, 139, 255];
const ACCENT: Color = [13, 71, 161, 255];
const ACCENT_SOFT: Color = [191, 219, 254, 255];
const HIGHLIGHT: Color = [21, 101, 52, 255];
const WARN: Color = [180, 83, 9, 255];
const KPI_LBL: Color = [71, 85, 105, 255];

struct Fonts<'a> {
    regular: FontRef<'a>,
    bold: FontRef<'a>,
}

#[derive(Clone, Copy)]
enum Style {
    Regular,
    Bold,
}

fn main() {
    let fonts = Fonts {
        regular: FontRef::try_from_slice(FONT_REGULAR).expect("regular font"),
        bold: FontRef::try_from_slice(FONT_BOLD).expect("bold font"),
    };

    // ---- Layout ----
    let margin = 44i32;
    let img_w: i32 = 1280;

    let title_size = 30.0f32;
    let subtitle_size = 17.5f32;
    let section_size = 18.5f32;
    let kpi_val_size = 34.0f32;
    let kpi_lbl_size = 13.5f32;
    let bar_label_size = 14.0f32;
    let bar_value_size = 13.0f32;
    let cell_size = 13.5f32;
    let header_size = 14.0f32;
    let footnote_size = 11.5f32;
    let line_gap = 5.5f32;

    // ---- Title / subtitle ----
    let title = "Indikationscode (IndC) — SL-Pflichtangabe ab 01.07.2026";
    let subtitle = "Aus dem BAG SL FHIR-Feed (Stand 06.2026) · BAG Rundschreiben vom 19.02.2026 \
                    · ab 01.01.2027 dürfen Versicherer Rechnungen ohne IndC zurückweisen";

    // ---- KPIs ----
    let kpis: [(&str, &str, Color); 4] = [
        ("1'419", "IndC-Zeilen im SL-Feed", ACCENT),
        ("571",   "unterschiedliche XXXXX.NN-Codes", ACCENT),
        ("264",   "BAG-Dossiernummern", HIGHLIGHT),
        ("77 %",  "der Codes in ATC L (Onkologie/Immun)", WARN),
    ];

    // ---- ATC distribution (count + percent) ----
    let atc: Vec<(&str, &str, i32, f32)> = vec![
        ("L", "Antineoplastika & Immunmodulatoren", 1094, 77.1),
        ("B", "Blut & blutbildende Organe",          123,  8.7),
        ("N", "Nervensystem",                         47,  3.3),
        ("A", "Alimentäres System & Stoffwechsel",   34,  2.4),
        ("C", "Kardiovaskuläres System",              30,  2.1),
        ("D", "Dermatologika",                        29,  2.0),
        ("J", "Antiinfektiva (systemisch)",           21,  1.5),
        ("R", "Atmungssystem",                        11,  0.8),
        ("S", "Sinnesorgane",                         10,  0.7),
        ("M", "Muskel-/Skelettsystem",                10,  0.7),
        ("H", "Hormone (systemisch)",                  5,  0.4),
        ("V", "Varia",                                 3,  0.2),
        ("G", "Urogenital & Sexualhormone",            2,  0.1),
    ];
    let atc_max = atc.iter().map(|x| x.2).max().unwrap_or(1) as f32;

    // ---- Sample IndC entries ----
    let sample_headers = ["IndC", "Markenname", "ATC", "Indikation (Auszug)"];
    let sample_widths: [i32; 4] = [120, 290, 90, 692];
    let sample_rows: Vec<[(&str, Style, Color); 4]> = vec![
        [
            ("17079.01", Style::Bold, ACCENT),
            ("MabThera Inf Konz 100 mg/10ml", Style::Bold, CELL_FG),
            ("L01FA01", Style::Regular, CELL_FG),
            ("Hämatologie — CD20+ follikuläres Non-Hodgkin-Lymphom (Stad. III–IV), Kombination mit CVP/CHOP", Style::Regular, CELL_FG),
        ],
        [
            ("17079.02", Style::Bold, ACCENT),
            ("MabThera Inf Konz 100 mg/10ml", Style::Regular, CELL_FG),
            ("L01FA01", Style::Regular, CELL_FG),
            ("Autoimmunerkrankungen — rheumatoide Arthritis u. weitere zugelassene Indikationen", Style::Regular, CELL_FG),
        ],
        [
            ("18082.01", Style::Bold, ACCENT),
            ("Avastin Inf Konz 100 mg/4ml", Style::Bold, CELL_FG),
            ("L01FG01", Style::Regular, CELL_FG),
            ("Kolorektalkarzinom", Style::Regular, CELL_FG),
        ],
        [
            ("18082.04", Style::Bold, ACCENT),
            ("Avastin Inf Konz 100 mg/4ml", Style::Regular, CELL_FG),
            ("L01FG01", Style::Regular, CELL_FG),
            ("Mammakarzinom", Style::Regular, CELL_FG),
        ],
        [
            ("18082.05", Style::Bold, ACCENT),
            ("Avastin Inf Konz 100 mg/4ml", Style::Regular, CELL_FG),
            ("L01FG01", Style::Regular, CELL_FG),
            ("Ovarialkarzinom", Style::Regular, CELL_FG),
        ],
        [
            ("Keytruda", Style::Bold, HIGHLIGHT),
            ("23 IndC-Codes auf einer einzigen Packung", Style::Regular, CELL_FG),
            ("L01FF02", Style::Regular, CELL_FG),
            ("Spitzenreiter: ein Pembrolizumab-Präparat trägt 23 verschiedene Indikationscodes — ein IndC pro vergüteter Indikation, jeder mit eigenem Rückerstattungsmodell.", Style::Regular, CELL_FG),
        ],
    ];

    // ---- Footnotes ----
    let footnotes = [
        "Quelle: BAG SL FHIR ndjson (epl.bag.admin.ch), ausgelesen mit cpp2sqlite (github.com/zdavatz/cpp2sqlite). Format Indikationscode: XXXXX.NN — fünfstellige BAG-Dossiernummer + zweistellige Indikations-Nummer.",
        "Hintergrund: bei Arzneimitteln mit Preismodell wird der SL-Listenpreis vergütet; ein Teil des Fabrikabgabepreises fliesst als Rückerstattung vom Pharmaunternehmen an den Versicherer zurück. Der IndC erlaubt die eindeutige Zuordnung Arzneimittel ↔ Indikation ↔ Rückerstattung.",
        "Fristen (BAG Rundschreiben vom 19.02.2026): 01.07.2026 — Übermittlung des IndC mit jeder Verordnung und Rechnung;  01.01.2027 — Krankenversicherer dürfen Rechnungen ohne IndC zurückweisen.",
    ];

    // ---- Pre-compute heights ----
    let lh = |size: f32| -> i32 { (size + line_gap).round() as i32 };

    let inner_w = img_w - margin * 2;

    let title_h = lh(title_size);
    let subtitle_wrap = wrap_text(&fonts, Style::Regular, subtitle_size, subtitle, inner_w);
    let subtitle_h = subtitle_wrap.len() as i32 * lh(subtitle_size);

    // KPI cards
    let kpi_gap = 16i32;
    let kpi_card_w = (inner_w - kpi_gap * 3) / 4;
    let kpi_card_h = 130i32;

    // Section: ATC distribution
    let atc_label_w = 320i32;
    let atc_value_w = 90i32;
    let atc_bar_x_off = atc_label_w + 14;
    let atc_bar_w_avail = inner_w - atc_bar_x_off - atc_value_w - 10;
    let atc_row_h = 26i32;
    let atc_block_h = atc.len() as i32 * atc_row_h;

    // Sample table
    let table_w: i32 = sample_widths.iter().sum();
    let cell_pad_x = 11i32;
    let cell_pad_y = 9i32;
    let header_lines: Vec<Vec<String>> = sample_headers
        .iter()
        .enumerate()
        .map(|(i, h)| wrap_text(&fonts, Style::Bold, header_size, h, sample_widths[i] - cell_pad_x * 2))
        .collect();
    let row_lines: Vec<Vec<Vec<String>>> = sample_rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, (t, s, _))| {
                    wrap_text(&fonts, *s, cell_size, t, sample_widths[i] - cell_pad_x * 2)
                })
                .collect()
        })
        .collect();
    let header_h = header_lines.iter().map(|l| l.len()).max().unwrap_or(1) as i32
        * lh(header_size)
        + cell_pad_y * 2;
    let row_heights: Vec<i32> = row_lines
        .iter()
        .map(|cells| {
            cells.iter().map(|l| l.len()).max().unwrap_or(1) as i32 * lh(cell_size)
                + cell_pad_y * 2
        })
        .collect();
    let table_h = header_h + row_heights.iter().sum::<i32>();

    let footnote_wrap: Vec<Vec<String>> = footnotes
        .iter()
        .map(|f| wrap_text(&fonts, Style::Regular, footnote_size, f, inner_w))
        .collect();
    let footnote_lines: i32 = footnote_wrap.iter().map(|l| l.len() as i32).sum();
    let footnote_h = footnote_lines * lh(footnote_size) + (footnotes.len() as i32 - 1) * 6;

    // ---- Vertical layout ----
    let mut y = margin;
    let title_y = y; y += title_h + 6;
    let subtitle_y = y; y += subtitle_h + 22;
    let kpi_y = y; y += kpi_card_h + 28;

    let atc_title_y = y; y += lh(section_size) + 12;
    let atc_y = y; y += atc_block_h + 28;

    let sample_title_y = y; y += lh(section_size) + 10;
    let table_y = y; y += table_h + 22;

    let footnotes_y = y; y += footnote_h + margin;

    let img_h = y as u32;

    // ---- Canvas ----
    let mut img = RgbaImage::from_pixel(img_w as u32, img_h, Rgba(BG));

    draw_text(&mut img, &fonts, Style::Bold, title_size, margin, title_y, title, TITLE_FG);
    draw_lines(
        &mut img, &fonts, Style::Regular, subtitle_size,
        margin, subtitle_y, &subtitle_wrap, line_gap, SUBTITLE_FG,
    );

    // ---- KPI cards ----
    let mut kx = margin;
    for (val, lbl, c) in &kpis {
        fill_rect(&mut img, kx, kpi_y, kpi_card_w, kpi_card_h, CARD_BG);
        draw_rect_outline(&mut img, kx, kpi_y, kpi_card_w, kpi_card_h, BORDER);
        // accent stripe
        fill_rect(&mut img, kx, kpi_y, 4, kpi_card_h, *c);
        // value
        draw_text(
            &mut img, &fonts, Style::Bold, kpi_val_size,
            kx + 22, kpi_y + 22, val, *c,
        );
        // label (wrap to 2 lines if needed)
        let lbl_wrap = wrap_text(&fonts, Style::Regular, kpi_lbl_size, lbl, kpi_card_w - 36);
        draw_lines(
            &mut img, &fonts, Style::Regular, kpi_lbl_size,
            kx + 22, kpi_y + 22 + lh(kpi_val_size) + 8, &lbl_wrap, 3.5, KPI_LBL,
        );
        kx += kpi_card_w + kpi_gap;
    }

    // ---- ATC bar chart ----
    draw_text(
        &mut img, &fonts, Style::Bold, section_size,
        margin, atc_title_y,
        "Verteilung der Indikationscodes nach ATC-Hauptklasse (n = 1'419)",
        TITLE_FG,
    );

    let bar_h = 16i32;
    for (i, (code, name, count, pct)) in atc.iter().enumerate() {
        let row_y = atc_y + i as i32 * atc_row_h;

        // label: "L  Antineoplastika & ..."
        let label = format!("{}   {}", code, name);
        draw_text(
            &mut img, &fonts, Style::Regular, bar_label_size,
            margin, row_y + (atc_row_h - lh(bar_label_size)) / 2,
            &label, CELL_FG,
        );

        // bar background
        let bar_y = row_y + (atc_row_h - bar_h) / 2;
        let bar_x = margin + atc_bar_x_off;
        fill_rect(&mut img, bar_x, bar_y, atc_bar_w_avail, bar_h, ACCENT_SOFT);
        // bar fill
        let fill_w = ((*count as f32 / atc_max) * atc_bar_w_avail as f32).round() as i32;
        fill_rect(&mut img, bar_x, bar_y, fill_w.max(2), bar_h, ACCENT);

        // numeric label
        let v = format!("{}  ({:.1} %)", count, pct);
        draw_text(
            &mut img, &fonts, Style::Bold, bar_value_size,
            bar_x + atc_bar_w_avail + 10, row_y + (atc_row_h - lh(bar_value_size)) / 2,
            &v, CELL_FG,
        );
    }

    // ---- Sample table ----
    draw_text(
        &mut img, &fonts, Style::Bold, section_size,
        margin, sample_title_y,
        "Beispiele aus dem Feed (von 1'419 Zeilen)",
        TITLE_FG,
    );

    let table_x = margin + (inner_w - table_w) / 2;

    fill_rect(&mut img, table_x, table_y, table_w, header_h, HEADER_BG);
    let mut cx = table_x;
    for (i, lines) in header_lines.iter().enumerate() {
        draw_lines(
            &mut img, &fonts, Style::Bold, header_size,
            cx + cell_pad_x, table_y + cell_pad_y,
            lines, line_gap, HEADER_FG,
        );
        cx += sample_widths[i];
    }

    let mut ry = table_y + header_h;
    for (r, cells) in row_lines.iter().enumerate() {
        let rh = row_heights[r];
        let bg = if r % 2 == 0 { ROW_A } else { ROW_B };
        fill_rect(&mut img, table_x, ry, table_w, rh, bg);
        let mut cx = table_x;
        for (i, lines) in cells.iter().enumerate() {
            let (_, style, color) = sample_rows[r][i];
            draw_lines(
                &mut img, &fonts, style, cell_size,
                cx + cell_pad_x, ry + cell_pad_y,
                lines, line_gap, color,
            );
            cx += sample_widths[i];
        }
        ry += rh;
    }

    // table grid
    draw_hline(&mut img, table_x, table_y, table_w, OUTER);
    draw_hline(&mut img, table_x, table_y + header_h, table_w, OUTER);
    draw_hline(&mut img, table_x, ry, table_w, OUTER);
    let mut yy = table_y + header_h;
    for r in 0..row_heights.len() - 1 {
        yy += row_heights[r];
        draw_hline(&mut img, table_x, yy, table_w, BORDER);
    }
    let mut vx = table_x;
    draw_vline(&mut img, vx, table_y, table_h, OUTER);
    for i in 0..sample_widths.len() {
        vx += sample_widths[i];
        let c = if i == sample_widths.len() - 1 { OUTER } else { BORDER };
        draw_vline(&mut img, vx, table_y, table_h, c);
    }

    // ---- Footnotes ----
    let mut fy = footnotes_y;
    for block in &footnote_wrap {
        draw_lines(
            &mut img, &fonts, Style::Regular, footnote_size,
            margin, fy, block, line_gap, NOTE_FG,
        );
        fy += block.len() as i32 * lh(footnote_size) + 6;
    }

    let out = "png/indc_overview.png";
    std::fs::create_dir_all("png").ok();
    img.save(out).expect("save png");
    println!("Wrote {} ({}x{})", out, img_w, img_h);
}

// ---- Text helpers (same primitives as src/main.rs) ----
fn text_width(fonts: &Fonts, style: Style, size: f32, text: &str) -> f32 {
    let font = match style {
        Style::Regular => &fonts.regular,
        Style::Bold => &fonts.bold,
    };
    let scaled = font.as_scaled(PxScale::from(size));
    let mut w = 0.0f32;
    let mut prev: Option<char> = None;
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

fn wrap_text(fonts: &Fonts, style: Style, size: f32, text: &str, max_w: i32) -> Vec<String> {
    let max_w = max_w as f32;
    let mut out = Vec::new();
    for segment in text.split('\n') {
        let mut line = String::new();
        for word in segment.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", line, word)
            };
            if text_width(fonts, style, size, &candidate) <= max_w || line.is_empty() {
                line = candidate;
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

fn fill_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Color) {
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    for yy in y..(y + h) {
        if yy < 0 || yy >= ih { continue; }
        for xx in x..(x + w) {
            if xx < 0 || xx >= iw { continue; }
            img.put_pixel(xx as u32, yy as u32, Rgba(color));
        }
    }
}

fn draw_hline(img: &mut RgbaImage, x: i32, y: i32, w: i32, color: Color) {
    fill_rect(img, x, y, w, 1, color);
}

fn draw_vline(img: &mut RgbaImage, x: i32, y: i32, h: i32, color: Color) {
    fill_rect(img, x, y, 1, h, color);
}

fn draw_rect_outline(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Color) {
    draw_hline(img, x, y, w, color);
    draw_hline(img, x, y + h - 1, w, color);
    draw_vline(img, x, y, h, color);
    draw_vline(img, x + w - 1, y, h, color);
}

fn draw_lines(
    img: &mut RgbaImage, fonts: &Fonts, style: Style, size: f32,
    x: i32, y: i32, lines: &[String], line_gap: f32, color: Color,
) {
    let step = (size + line_gap).round() as i32;
    for (i, line) in lines.iter().enumerate() {
        draw_text(img, fonts, style, size, x, y + i as i32 * step, line, color);
    }
}

fn draw_text(
    img: &mut RgbaImage, fonts: &Fonts, style: Style, size: f32,
    x: i32, y: i32, text: &str, color: Color,
) {
    let font = match style {
        Style::Regular => &fonts.regular,
        Style::Bold => &fonts.bold,
    };
    let scale = PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();
    let baseline_y = y as f32 + ascent;
    let mut caret = x as f32;
    let mut prev: Option<char> = None;
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    for c in text.chars() {
        let gid = font.glyph_id(c);
        if let Some(p) = prev {
            caret += scaled.kern(font.glyph_id(p), gid);
        }
        let glyph: Glyph = gid.with_scale_and_position(scale, ab_glyph::point(caret, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= iw || py >= ih { return; }
                let bgp = img.get_pixel(px as u32, py as u32).0;
                let a = cov.clamp(0.0, 1.0) * (color[3] as f32 / 255.0);
                let blended = [
                    blend(bgp[0], color[0], a),
                    blend(bgp[1], color[1], a),
                    blend(bgp[2], color[2], a),
                    255,
                ];
                img.put_pixel(px as u32, py as u32, Rgba(blended));
            });
        }
        caret += scaled.h_advance(gid);
        prev = Some(c);
    }
}

fn blend(bg: u8, fg: u8, a: f32) -> u8 {
    (fg as f32 * a + bg as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8
}
