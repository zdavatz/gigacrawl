use ab_glyph::{Font, FontRef, Glyph, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

mod linkedin;

// ---- Font handles (DejaVu Sans available on this system) ----
const FONT_REGULAR: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf");

type Color = [u8; 4];

// ---- Palette ----
const BG: Color = [248, 250, 252, 255];
const TITLE_FG: Color = [15, 23, 42, 255];
const SUBTITLE_FG: Color = [71, 85, 105, 255];
const HEADER_BG: Color = [30, 58, 95, 255];
const HEADER_FG: Color = [255, 255, 255, 255];
const ROW_A: Color = [255, 255, 255, 255];
const ROW_B: Color = [237, 242, 248, 255];
const CELL_FG: Color = [30, 41, 59, 255];
const COMPANY_FG: Color = [12, 74, 110, 255];
const NOTE_FG: Color = [71, 85, 105, 255];
const CAPEX_FG: Color = [21, 101, 52, 255];
const SITE_FG: Color = [55, 48, 107, 255];
const BORDER: Color = [203, 213, 225, 255];
const OUTER_BORDER: Color = [148, 163, 184, 255];
const FOOTNOTE_FG: Color = [100, 116, 139, 255];

struct Fonts<'a> {
    regular: FontRef<'a>,
    bold: FontRef<'a>,
}

#[derive(Clone, Copy)]
enum Style {
    Regular,
    Bold,
}

struct Cell {
    text: &'static str,
    style: Style,
    color: Color,
}

impl Cell {
    fn new(text: &'static str, style: Style, color: Color) -> Self {
        Cell { text, style, color }
    }
}

const NCOL: usize = 6;

fn main() {
    // CLI: `--auth` runs the LinkedIn OAuth flow; `--post-linkedin` posts the
    // rendered chart after generating it. No flag = just render the PNG.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--auth") {
        if let Err(e) = linkedin::authenticate() {
            eprintln!("[linkedin] auth failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    let post = args.iter().any(|a| a == "--post-linkedin" || a == "--post");

    let fonts = Fonts {
        regular: FontRef::try_from_slice(FONT_REGULAR).expect("regular font"),
        bold: FontRef::try_from_slice(FONT_BOLD).expect("bold font"),
    };

    // ---- Layout constants ----
    let margin = 40i32;
    let title_size = 31.0f32;
    let subtitle_size = 18.0f32;
    let header_size = 15.5f32;
    let cell_size = 14.0f32;
    let footnote_size = 12.5f32;
    let line_gap = 6.0f32; // extra space between wrapped lines
    let cell_pad_x = 12i32;
    let cell_pad_y = 11i32;

    // ---- Column widths ----
    let col_w: [i32; NCOL] = [128, 162, 196, 116, 330, 332];
    let table_w: i32 = col_w.iter().sum();
    let img_w = (table_w + margin * 2) as u32;

    // ---- Table content ----
    let headers: [&str; NCOL] = [
        "Company",
        "Operational\n(Up & Running)",
        "Planned / Under Construction",
        "FY2025 Capex\n(per 10-K)",
        "Key Sites & Power (location · GW)",
        "Key Notes",
    ];

    // Rows are ordered by estimated operational GW, descending.
    let rows: Vec<[Cell; NCOL]> = vec![
        [
            Cell::new("Amazon (AWS)", Style::Bold, COMPANY_FG),
            Cell::new("~10–15+ GW (global est.)", Style::Regular, CELL_FG),
            Cell::new(
                "Multi-GW additions ongoing (on track to double current capacity by 2027)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$128.3B", Style::Bold, CAPEX_FG),
            Cell::new(
                "New Carlisle, IN ($11–15B; ~2.4 GW added in region) · N. Virginia (~2.75 GW; $35B through 2040)",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Added 3.8 GW in the past 12 months. 2.2 GW Indiana campus partially operational. 10-K: capex \"expected to increase in 2026\".",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("Microsoft (Azure)", Style::Bold, COMPANY_FG),
            Cell::new("~5–8+ GW (global, est.)", Style::Regular, CELL_FG),
            Cell::new("Large pipeline (multi-GW projects)", Style::Regular, CELL_FG),
            Cell::new("$64.6B", Style::Bold, CAPEX_FG),
            Cell::new(
                "Fairwater — Wisconsin (~$7.3B; ~0.9 GW build-out, online early 2026) · Fairwater Atlanta (online) · Fairwater 4 under constr.",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Added ~2 GW FY2025 + ~1 GW Q2 FY2026. FY ends June. 10-K: will \"continue to invest\" in AI infrastructure.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("Google (Cloud)", Style::Bold, COMPANY_FG),
            Cell::new("Several GW (global, est.)", Style::Regular, CELL_FG),
            Cell::new(
                "Significant expansions (e.g., 1 GW+ demand response deals)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$91.4B", Style::Bold, CAPEX_FG),
            Cell::new(
                "Global fleet. $52.7B of long-term data-center leases signed but not yet commenced (10-K)",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "10-K: expects to \"significantly increase\" 2026 technical-infrastructure investment vs 2025, incl. data centers.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("Meta", Style::Bold, COMPANY_FG),
            Cell::new("Several GW operational", Style::Regular, CELL_FG),
            Cell::new(
                "Prometheus (~1 GW online in 2026)\nHyperion (phased to 5 GW long-term; 2 GW by ~2030)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$69.7B¹", Style::Bold, CAPEX_FG),
            Cell::new(
                "Prometheus — New Albany, OH (~1 GW, online 2026) · Hyperion — Richland Parish, LA (→5 GW; $27B Blue Owl JV, 2,250 acres)",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "10-K guides FY2026 capex to ~$115–135B.¹ Hyperion is one of the largest planned campuses worldwide.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("xAI", Style::Bold, COMPANY_FG),
            Cell::new("~2 GW (Colossus, Memphis)", Style::Regular, CELL_FG),
            Cell::new(
                "Further expansions (roadmap to much larger)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("Private — n/a", Style::Regular, NOTE_FG),
            Cell::new(
                "Memphis, TN — Colossus 1 (~0.3 GW) + Colossus 2 (~1.2 GW → ~2 GW, 555k+ GPUs); on-site power hub in Southaven, MS",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Colossus 2 is among the first ~GW-scale single sites. Colossus 1 output now committed to Anthropic ($1.25B/mo through 2029).",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("OpenAI", Style::Bold, COMPANY_FG),
            Cell::new(
                "~0.3 GW (Stargate Abilene, partial) + Azure access",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new(
                "Stargate: ~7–10 GW planned ($500B); 4.5 GW Oracle agreement",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("Private — $500B plan", Style::Regular, NOTE_FG),
            Cell::new(
                "Abilene, TX (flagship → 1.2 GW; ~0.3 GW live) · Shackelford Co., TX · Doña Ana Co., NM · Lordstown, OH · Wisconsin · UAE (2026)",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Stargate JV with SoftBank & Oracle (+ CoreWeave). Targets ~10 GW / $500B by 2029; >3 GW added in early 2026.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("Anthropic", Style::Bold, COMPANY_FG),
            Cell::new(
                "Limited owned capacity (mostly partner access)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new(
                "Multi-GW access via partners (1+ GW coming online 2026–2027)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("Private — $50B US plan", Style::Regular, NOTE_FG),
            Cell::new(
                "Own (Fluidstack): Abernathy, TX (~168 MW) · Lake Mariner, NY (~360 MW). Partners: AWS, Google, Azure, xAI/Colossus 1",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "$50B US infrastructure plan, sites online through 2026. Also exploring multi-GW orbital (space-based) compute with SpaceX.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
    ];

    let footnotes: [&str; 2] = [
        "¹ Meta 10-K (FY2025): \"We anticipate making capital expenditures of approximately $115 billion to $135 billion in 2026 to support our AI efforts and core business.\"",
        "Capex = purchases of property & equipment from the latest annual 10-K cash-flow statement (Microsoft FY ends June; Amazon/Alphabet/Meta FY ends December). xAI & Anthropic are private and do not file with the SEC. GW capacity figures and site details are press/analyst-sourced — SEC filings do not disclose capacity in gigawatts.",
    ];

    // ---- Pre-compute wrapped lines per cell to derive row heights ----
    let header_lines: Vec<Vec<String>> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| wrap_text(&fonts, Style::Bold, header_size, h, col_w[i] - cell_pad_x * 2))
        .collect();

    let row_lines: Vec<Vec<Vec<String>>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, c)| {
                    wrap_text(&fonts, c.style, cell_size, c.text, col_w[i] - cell_pad_x * 2)
                })
                .collect()
        })
        .collect();

    let line_h = |size: f32| -> i32 { (size + line_gap).round() as i32 };

    let header_h = {
        let max_lines = header_lines.iter().map(|l| l.len()).max().unwrap_or(1) as i32;
        max_lines * line_h(header_size) + cell_pad_y * 2
    };

    let row_heights: Vec<i32> = row_lines
        .iter()
        .map(|cells| {
            let max_lines = cells.iter().map(|l| l.len()).max().unwrap_or(1) as i32;
            max_lines * line_h(cell_size) + cell_pad_y * 2
        })
        .collect();

    // ---- Vertical layout ----
    let title_y = margin;
    let title_h = line_h(title_size);
    let subtitle_y = title_y + title_h + 4;
    let subtitle_h = line_h(subtitle_size);
    let table_y = subtitle_y + subtitle_h + 22;

    let table_h: i32 = header_h + row_heights.iter().sum::<i32>();

    // Footnote block (wrapped across full table width).
    let footnote_wrapped: Vec<Vec<String>> = footnotes
        .iter()
        .map(|f| wrap_text(&fonts, Style::Regular, footnote_size, f, table_w))
        .collect();
    let footnote_lines: i32 = footnote_wrapped.iter().map(|l| l.len() as i32).sum();
    let footnotes_y = table_y + table_h + 16;
    let footnotes_h = footnote_lines * line_h(footnote_size) + (footnotes.len() as i32 - 1) * 4;

    let img_h = (footnotes_y + footnotes_h + margin) as u32;

    // ---- Canvas ----
    let mut img = RgbaImage::from_pixel(img_w, img_h, Rgba(BG));

    // ---- Title & subtitle ----
    draw_text(
        &mut img,
        &fonts,
        Style::Bold,
        title_size,
        margin,
        title_y,
        "Data Center Power Capacity (GW) — Operational vs. Planned, with SEC Capex",
        TITLE_FG,
    );
    draw_text(
        &mut img,
        &fonts,
        Style::Regular,
        subtitle_size,
        margin,
        subtitle_y,
        "as of mid-2026  ·  capex & PP&E from FY2025 SEC 10-K filings",
        SUBTITLE_FG,
    );

    // ---- Table ----
    let table_x = margin;

    // Header background
    fill_rect(&mut img, table_x, table_y, table_w, header_h, HEADER_BG);

    // Header text
    let mut cx = table_x;
    for (i, lines) in header_lines.iter().enumerate() {
        draw_lines(
            &mut img,
            &fonts,
            Style::Bold,
            header_size,
            cx + cell_pad_x,
            table_y + cell_pad_y,
            lines,
            line_gap,
            HEADER_FG,
        );
        cx += col_w[i];
    }

    // Rows
    let mut ry = table_y + header_h;
    for (r, cells) in row_lines.iter().enumerate() {
        let rh = row_heights[r];
        let bg = if r % 2 == 0 { ROW_A } else { ROW_B };
        fill_rect(&mut img, table_x, ry, table_w, rh, bg);

        let mut cx = table_x;
        for (i, lines) in cells.iter().enumerate() {
            let cell = &rows[r][i];
            draw_lines(
                &mut img,
                &fonts,
                cell.style,
                cell_size,
                cx + cell_pad_x,
                ry + cell_pad_y,
                lines,
                line_gap,
                cell.color,
            );
            cx += col_w[i];
        }
        ry += rh;
    }

    // ---- Grid lines ----
    draw_hline(&mut img, table_x, ry, table_w, OUTER_BORDER); // bottom
    draw_hline(&mut img, table_x, table_y, table_w, OUTER_BORDER); // top
    draw_hline(&mut img, table_x, table_y + header_h, table_w, OUTER_BORDER); // header sep
    let mut yy = table_y + header_h;
    for r in 0..row_heights.len() - 1 {
        yy += row_heights[r];
        draw_hline(&mut img, table_x, yy, table_w, BORDER);
    }

    // Vertical column separators + outer left/right
    let mut vx = table_x;
    draw_vline(&mut img, vx, table_y, table_h, OUTER_BORDER);
    for i in 0..col_w.len() {
        vx += col_w[i];
        let color = if i == col_w.len() - 1 {
            OUTER_BORDER
        } else {
            BORDER
        };
        draw_vline(&mut img, vx, table_y, table_h, color);
    }

    // ---- Footnotes ----
    let mut fy = footnotes_y;
    for block in &footnote_wrapped {
        draw_lines(
            &mut img,
            &fonts,
            Style::Regular,
            footnote_size,
            table_x,
            fy,
            block,
            line_gap,
            FOOTNOTE_FG,
        );
        fy += block.len() as i32 * line_h(footnote_size) + 4;
    }

    let out = "png/datacenter_capacity.png";
    img.save(out).expect("save png");
    println!("Wrote {} ({}x{})", out, img_w, img_h);

    if post {
        match linkedin::publish_image(std::path::Path::new(out)) {
            Ok(url) => println!("Posted to LinkedIn: {url}"),
            Err(e) => {
                eprintln!("[linkedin] post failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

// ---- Text wrapping ----
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
    // Honour explicit newlines first.
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

// ---- Drawing primitives ----
fn fill_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Color) {
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    for yy in y..(y + h) {
        if yy < 0 || yy >= ih {
            continue;
        }
        for xx in x..(x + w) {
            if xx < 0 || xx >= iw {
                continue;
            }
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

fn draw_lines(
    img: &mut RgbaImage,
    fonts: &Fonts,
    style: Style,
    size: f32,
    x: i32,
    y: i32,
    lines: &[String],
    line_gap: f32,
    color: Color,
) {
    let step = (size + line_gap).round() as i32;
    for (i, line) in lines.iter().enumerate() {
        draw_text(img, fonts, style, size, x, y + i as i32 * step, line, color);
    }
}

/// Draw a single line of text. `y` is the top of the line box.
fn draw_text(
    img: &mut RgbaImage,
    fonts: &Fonts,
    style: Style,
    size: f32,
    x: i32,
    y: i32,
    text: &str,
    color: Color,
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
                if px < 0 || py < 0 || px >= iw || py >= ih {
                    return;
                }
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
    (fg as f32 * a + bg as f32 * (1.0 - a))
        .round()
        .clamp(0.0, 255.0) as u8
}
