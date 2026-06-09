use ab_glyph::{Font, FontRef, Glyph, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

mod linkedin;
mod twitter;

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
const COSTGW_FG: Color = [146, 64, 14, 255];
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

const NCOL: usize = 7;

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
    if let Some(i) = args.iter().position(|a| a == "--delete-tweet") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        if id.is_empty() {
            eprintln!("--delete-tweet requires a tweet ID");
            std::process::exit(1);
        }
        if let Err(e) = twitter::delete_tweet(&id) {
            eprintln!("[twitter] delete failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    // `--post-sec`: render page 2 of the PDF (the SEC financials table) to a PNG
    // and post it to LinkedIn + X with an SEC-specific caption.
    if args.iter().any(|a| a == "--post-sec") {
        let sec_png = "png/sec_financials.png";
        let rendered = std::process::Command::new("pdftoppm")
            .args([
                "-png", "-r", "200", "-f", "2", "-l", "2", "-singlefile",
                "pdf/datacenter_sources.pdf", "png/sec_financials",
            ])
            .status();
        match rendered {
            Ok(s) if s.success() => println!("Rendered {sec_png}"),
            _ => {
                eprintln!("Failed to render page 2 (need `pdftoppm` and pdf/datacenter_sources.pdf)");
                std::process::exit(1);
            }
        }
        let path = std::path::Path::new(sec_png);
        let li_caption = "How much are the AI hyperscalers actually spending? Straight from the FY2025 SEC 10-Ks:\n\nFY2025 capex — Amazon $131.8B, Alphabet $91.4B, Meta $69.7B, Microsoft $64.6B, Oracle $21.2B.\nAlso PP&E (net), operating cash flow, capex÷OCF, and \"leases not yet commenced\" (mostly data centers): Amazon $96.4B, Microsoft $92.7B, Meta $103.8B, Alphabet $58.5B, Oracle $43.4B.\n\nEvery figure links to the underlying 10-K on sec.gov. Full clickable PDF:\ngithub.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf\n#AI #DataCenters #CapEx #SEC #CloudInfrastructure";
        let tw_caption = "FY2025 AI data-center capex from the SEC 10-Ks: Amazon $131.8B · Alphabet $91.4B · Meta $69.7B · Microsoft $64.6B · Oracle $21.2B. Plus PP&E, operating cash flow & \"leases not yet commenced\" — each figure links to its filing.\ngithub.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf\n#AI #SEC #CapEx";
        let title = "AI Data-Center Capex — FY2025 SEC 10-K Financials";
        match linkedin::publish_image(path, li_caption, title) {
            Ok(u) => println!("Posted SEC page to LinkedIn: {u}"),
            Err(e) => eprintln!("[linkedin] post failed: {e}"),
        }
        match twitter::publish_image(path, tw_caption) {
            Ok(u) => println!("Posted SEC page to X: {u}"),
            Err(e) => eprintln!("[twitter] post failed: {e}"),
        }
        return;
    }

    // `--post-pdf-doc`: post the PDF itself to LinkedIn as a NATIVE document
    // post (Documents API) — renders as a swipeable, downloadable carousel.
    // Unlike the image posts, this keeps the actual PDF; note LinkedIn's in-feed
    // viewer rasterizes pages, so the per-figure 10-K links are clickable only
    // after download. LinkedIn only (X accepts no PDFs).
    if args.iter().any(|a| a == "--post-pdf-doc") {
        let pdf_path = std::path::Path::new("pdf/datacenter_sources.pdf");
        if !pdf_path.exists() {
            eprintln!("pdf/datacenter_sources.pdf not found — run `datacenter_pdf` first");
            std::process::exit(1);
        }
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf";
        let caption = format!(
            "The AI data-center buildout, in one document — capacity, SEC financials, and the private players. Swipe through all five pages:\n\n\
            New: xAI's data centers finally show up in SEC filings — twice. SpaceX's IPO FWP (5 Jun 2026) disclosed the Google cloud deal ($920M/month for ~110,000 Nvidia GPUs, Oct 2026–Jun 2029, ~$30B over the term). And its EU retail prospectus (FWP, 8 Jun 2026, BaFin-approved, also passported to Swiss retail) now quantifies the hardware: Colossus I+II ≈1.0 GW of compute power — first cluster of 100k H100 (~130 MW) online in 122 days, first 110k GB200 (~210 MW) in 91 days, ≥220k GB300 and >400 MW more coming. The first GW capacity figure for xAI in an SEC-filed document — until now, every GW number for the private operators was press/analyst estimate. Linked on pages 1, 4 and 5.\n\n\
            1/ Power capacity (GW), operational vs. planned, with FY2025 capex & est. $/GW — Amazon, Microsoft, Google, Meta, Oracle, xAI, OpenAI, Anthropic.\n\
            2/ The SEC 10-K financials: capex FY23–25, PP&E, operating cash flow, capex÷OCF and \"leases not yet commenced\". Incl. Oracle (RPO backlog $553B; FY26 capex ~$50B).\n\
            3/ Where the capital actually sits — compute/servers vs. real estate vs. construction-in-progress, straight from each property & equipment note.\n\
            4/ The private players (xAI, OpenAI, Anthropic): press/analyst estimates of GPUs/silicon vs. construction/power/land — plus the SpaceX FWP exception above.\n\
            5/ Off-grid vs on-grid: what the SEC filings actually disclose about how the data centers are powered. Capacity is now SEC-quantified by three (CoreWeave, Nebius, xAI via SpaceX) — but still not one filer states a grid split, and the ~56 GW behind-the-meter gas build-out remains invisible to the filings.\n\n\
            Every figure links to its SEC filing — download the PDF (here, or on GitHub) to click through: {pdf_url}\n\
            #AI #DataCenters #CapEx #SEC"
        );
        let title = "AI Data-Center Capacity & SEC Financials";
        match linkedin::publish_document(pdf_path, &caption, title) {
            Ok(u) => println!("Posted PDF document to LinkedIn: {u}"),
            Err(e) => {
                eprintln!("[linkedin] document post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-spacex-doc`: post pdf/spacex_exposure.pdf to LinkedIn as a NATIVE
    // document (Documents API), identical mechanism to --post-pdf-doc but with
    // the SpaceX-exposure caption. LinkedIn only.
    if args.iter().any(|a| a == "--post-spacex-doc") {
        let pdf_path = std::path::Path::new("pdf/spacex_exposure.pdf");
        if !pdf_path.exists() {
            eprintln!("pdf/spacex_exposure.pdf not found — run `spacex_exposure` first");
            std::process::exit(1);
        }
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/spacex_exposure.pdf";
        let caption = format!(
            "Wie kommt man an SpaceX, bevor es an die Börse geht? 🚀\n\n\
            SpaceX ist privat und reicht bei der SEC nur Form D ein — aber eine Reihe börsengehandelter und offener Fonds hält bereits SpaceX-Anteile und legt sie in ihren SEC-Filings (Form N-PORT / N-CSR) offen. Damit gibt es heute schon „Pre-IPO\"-Exposure.\n\n\
            Wir haben die Filings durchsucht (EDGAR-Volltextsuche nach „Space Exploration Technologies\") und jede Position verifiziert. Sortiert nach SpaceX-Anteil am Fonds:\n\n\
            • Destiny Tech100 (NYSE: DXYZ) — ~37 % des NAV, der reinste börsennotierte SpaceX-Proxy\n\
            • Baron Partners (BPTRX) & Baron Focused Growth (BFGFX) — SpaceX ist die grösste Position (~13–14 %)\n\
            • ARK Venture (ARKVX), StepStone, The Private Shares Fund (PIIVX), Coatue\n\
            • BlackRock Capital Allocation Trust (NYSE: BCAT) — börsennotiert, aber nur winzige Gewichtung\n\n\
            Jede Zeile verlinkt direkt auf das zugrunde liegende SEC-Filing. Plus eine ehrliche Fussnote zu Alphabets 900-Mio.-Investment von 2015 (heute nicht mehr transparent ausgewiesen).\n\n\
            📄 Volles klickbares PDF (alle Quellen verlinkt): {pdf_url}\n\n\
            Keine Anlageberatung, nur persönliche Analyse.\n\
            #SpaceX #PreIPO #SEC #Investing #PrivateMarkets"
        );
        let title = "SpaceX vor dem IPO — börsennotierte Fonds, die es halten (laut SEC)";
        match linkedin::publish_document(pdf_path, &caption, title) {
            Ok(u) => println!("Posted SpaceX PDF document to LinkedIn: {u}"),
            Err(e) => {
                eprintln!("[linkedin] document post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-indc-doc`: post pdf/indc_overview.pdf to LinkedIn as a NATIVE
    // document (Documents API), identical mechanism to --post-pdf-doc but with
    // the IndC caption. LinkedIn only.
    if args.iter().any(|a| a == "--post-indc-doc") {
        let pdf_path = std::path::Path::new("pdf/indc_overview.pdf");
        if !pdf_path.exists() {
            eprintln!("pdf/indc_overview.pdf not found — run `indc_pdf` first");
            std::process::exit(1);
        }
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/indc_overview.pdf";
        let xlsx_url = "github.com/zdavatz/gigacrawl/blob/main/xlsx/indc.xlsx";
        let ios_url = "apps.apple.com/ch/app/generika/id520038123";
        let play_url = "play.google.com/store/apps/details?id=org.oddb.generika";
        let caption = format!(
            "🇨🇭 Ab 01.07.2026 wird der Indikationscode (IndC) zur SL-Pflichtangabe — ab 01.01.2027 dürfen Versicherer Rechnungen ohne ihn zurückweisen.\n\n\
            Wir haben uns angesehen, was das in der Praxis bedeutet, und den BAG-SL-FHIR-Feed mit unserem Open-Source-Werkzeug cpp2sqlite durchgekämmt:\n\n\
            • 1'419 IndC-Zeilen, 571 distinkte XXXXX.NN-Codes, 264 BAG-Dossiernummern, 524 Markennamen\n\
            • 77 % der Codes entfallen auf ATC L (Onkologie / Immunmodulatoren) — genau jener Bereich mit den höchsten Rückerstattungen\n\
            • Preisspanne der betroffenen Präparate: CHF 3.30 – 82'239.72 ex-factory (Faktor ~25'000)\n\
            • Spitzenreiter bei den Codes: ein einziges Keytruda-Präparat trägt 23 unterschiedliche IndC — einer pro vergüteter Indikation, jeder mit eigenem Preismodell\n\n\
            Hintergrund: Bei Arzneimitteln mit Preismodell vergütet die Kasse den SL-Listenpreis, ein Teil des FAP fliesst nachträglich vom Pharmaunternehmen zurück. Der IndC ist die einzige saubere Brücke zwischen Arzneimittel ↔ Indikation ↔ Rückerstattung — und damit Voraussetzung der Wirtschaftlichkeit nach KVG Art. 42 Abs. 3.\n\n\
            Wir haben den IndC bereits End-to-End integriert:\n\
            – im Datenpipeline-Tool cpp2sqlite → SQLite-Spalten indikationscode / indikationscode_text\n\
            – in der Generika.cc App (iOS + Android, ywesee GmbH): das eingebaute Kostengutsprache-Formular (KVV 71) zieht die IndC live aus der SL, zeigt den Limitations-Text inline und bettet die Auswahl in den PDF-/E-Mail-Versand ein. Bereit ab dem 01.07.2026 — ohne dass Praxen den Code von Hand aus der ePL kopieren müssen.\n\n\
            📄 Volle Übersicht als klickbares PDF (mit allen Quellen-Links): {pdf_url}\n\
            📊 Rohdaten (xlsx): {xlsx_url}\n\
            📱 App: {ios_url} · {play_url}\n\n\
            #Spezialitätenliste #Indikationscode #BAG #Krankenversicherung #DigitalHealth #OpenSource #Pharma #Schweiz"
        );
        let title = "Indikationscode (IndC) — SL-Pflichtangabe ab 01.07.2026";
        match linkedin::publish_document(pdf_path, &caption, title) {
            Ok(u) => println!("Posted IndC PDF document to LinkedIn: {u}"),
            Err(e) => {
                eprintln!("[linkedin] document post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-indc-png`: post png/indc_overview.png to LinkedIn as a standard
    // image post with the IndC caption (mirrors --post-indc-doc but with the
    // PNG instead of the native PDF document).
    if args.iter().any(|a| a == "--post-indc-png") {
        let path = std::path::Path::new("png/indc_overview.png");
        if !path.exists() {
            eprintln!("png/indc_overview.png not found — run `indc_chart` first");
            std::process::exit(1);
        }
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/indc_overview.pdf";
        let xlsx_url = "github.com/zdavatz/gigacrawl/blob/main/xlsx/indc.xlsx";
        let ios_url = "apps.apple.com/ch/app/generika/id520038123";
        let play_url = "play.google.com/store/apps/details?id=org.oddb.generika";
        let caption = format!(
            "🇨🇭 Ab 01.07.2026 wird der Indikationscode (IndC) zur SL-Pflichtangabe — ab 01.01.2027 dürfen Versicherer Rechnungen ohne ihn zurückweisen.\n\n\
            Wir haben den BAG-SL-FHIR-Feed mit unserem Open-Source-Werkzeug cpp2sqlite durchgekämmt:\n\n\
            • 1'419 IndC-Zeilen, 571 distinkte XXXXX.NN-Codes, 264 BAG-Dossiernummern, 524 Markennamen\n\
            • 77 % der Codes entfallen auf ATC L (Onkologie / Immunmodulatoren)\n\
            • Preisspanne der betroffenen Präparate: CHF 3.30 – 82'239.72 ex-factory (Faktor ~25'000)\n\
            • Ein einziges Keytruda-Präparat trägt 23 unterschiedliche IndC — einer pro vergüteter Indikation, jeder mit eigenem Preismodell\n\n\
            Bei Arzneimitteln mit Preismodell vergütet die Kasse den SL-Listenpreis, ein Teil des FAP fliesst nachträglich vom Pharmaunternehmen zurück. Der IndC ist die einzige saubere Brücke zwischen Arzneimittel ↔ Indikation ↔ Rückerstattung.\n\n\
            Wir haben den IndC bereits End-to-End integriert:\n\
            – im Datenpipeline-Tool cpp2sqlite → SQLite-Spalten indikationscode / indikationscode_text\n\
            – in der Generika.cc App (iOS + Android, ywesee GmbH): das eingebaute Kostengutsprache-Formular (KVV 71) zieht die IndC live aus der SL, zeigt den Limitations-Text inline und bettet die Auswahl in den PDF-/E-Mail-Versand ein. Bereit ab 01.07.2026 — ohne dass Praxen den Code von Hand aus der ePL kopieren müssen.\n\n\
            📄 PDF-Übersicht (mit klickbaren Quellen): {pdf_url}\n\
            📊 Rohdaten (xlsx): {xlsx_url}\n\
            📱 App: {ios_url} · {play_url}\n\n\
            #Spezialitätenliste #Indikationscode #BAG #Krankenversicherung #DigitalHealth #OpenSource #Pharma #Schweiz"
        );
        let title = "Indikationscode (IndC) — SL-Pflichtangabe ab 01.07.2026";
        match linkedin::publish_image(path, &caption, title) {
            Ok(u) => println!("Posted IndC PNG to LinkedIn: {u}"),
            Err(e) => {
                eprintln!("[linkedin] post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-png <path> <caption>`: post a single PNG as a standalone tweet
    // with the given caption. General-purpose; used e.g. to post individual PDF
    // pages on X pay-per-use, which only permits plain single-image posts.
    if let Some(i) = args.iter().position(|a| a == "--post-png") {
        let path = args.get(i + 1).cloned().unwrap_or_default();
        let caption = args.get(i + 2).cloned().unwrap_or_default();
        if path.is_empty() || caption.is_empty() {
            eprintln!("--post-png requires <path> <caption>");
            std::process::exit(1);
        }
        match twitter::publish_image(std::path::Path::new(&path), &caption) {
            Ok(u) => println!("Posted {path} to X: {u}"),
            Err(e) => {
                eprintln!("[twitter] post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-pdf-thread`: rasterize all five PDF pages and post them to X as a
    // reply-chain THREAD (one single-image tweet per page). This is the
    // workaround for X pay-per-use rejecting *multi*-image posts (403) while
    // single-image posts succeed.
    if args.iter().any(|a| a == "--post-pdf-thread") {
        let pages = ["png/pdf_page-1.png", "png/pdf_page-2.png", "png/pdf_page-3.png", "png/pdf_page-4.png", "png/pdf_page-5.png"];
        let rendered = std::process::Command::new("pdftoppm")
            .args(["-png", "-r", "200", "pdf/datacenter_sources.pdf", "png/pdf_page"])
            .status();
        match rendered {
            Ok(s) if s.success() => println!("Rendered {} PDF pages", pages.len()),
            _ => {
                eprintln!("Failed to rasterize the PDF (need `pdftoppm` and pdf/datacenter_sources.pdf)");
                std::process::exit(1);
            }
        }
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf";
        let captions = [
            format!(
                "🧵 The AI data-center buildout, in five charts (full clickable PDF at the end).\n\n\
                1/ Power capacity (GW) — operational vs. planned — with FY2025 capex & est. $/GW: Amazon, Microsoft, Google, Meta, Oracle, xAI, OpenAI, Anthropic.\n\
                #AI #DataCenters #CapEx"
            ),
            format!(
                "2/ The SEC 10-K financials: capex FY23–25, PP&E, operating cash flow, capex÷OCF and \"leases not yet commenced\". Now incl. Oracle (RPO backlog $553B; FY26 capex ~$50B)."
            ),
            format!(
                "3/ Where the capital actually sits — compute/servers vs. real estate vs. construction-in-progress, straight from each property & equipment note. SEC filings do not isolate GPU spend."
            ),
            format!(
                "4/ The private players (xAI, OpenAI, Anthropic): press/analyst estimates of GPUs/silicon vs. construction/power/land. It's why xAI's plant looks cheap — they cut the facility cost, but the GPUs still dominate the all-in."
            ),
            format!(
                "5/ Off-grid vs on-grid: what the 10-Ks actually disclose about how these data centers are powered — essentially nothing. 0 of 7 SEC filers state grid vs self-generation; the ~56 GW behind-the-meter gas build-out (xAI, Meta's Williams plant, Stargate) is invisible to SEC filings.\n\n\
                Full clickable PDF (every figure links to its SEC 10-K):\n{pdf_url}"
            ),
        ];
        let items: Vec<(&std::path::Path, String)> = pages
            .iter()
            .zip(captions)
            .map(|(p, c)| (std::path::Path::new(*p), c))
            .collect();
        match twitter::publish_thread(&items) {
            Ok(u) => println!("Posted all 5 PDF pages to X as a thread: {u}"),
            Err(e) => {
                eprintln!("[twitter] thread post failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // `--post-pdf`: rasterize all five PDF pages to PNGs and post them as a
    // single multi-image post — to LinkedIn (multiImage, up to 20) and,
    // best-effort, to X (which caps at 4 images and currently 403s on
    // pay-per-use writes anyway, so it gets the first 4 pages). Always links to
    // the full PDF on GitHub. `--post-pdf-x` restricts to X only; default does both.
    if args.iter().any(|a| a == "--post-pdf" || a == "--post-pdf-x" || a == "--post-pdf-li") {
        let x_only = args.iter().any(|a| a == "--post-pdf-x");
        let li_only = args.iter().any(|a| a == "--post-pdf-li");
        let pages = ["png/pdf_page-1.png", "png/pdf_page-2.png", "png/pdf_page-3.png", "png/pdf_page-4.png", "png/pdf_page-5.png"];
        let rendered = std::process::Command::new("pdftoppm")
            .args([
                "-png", "-r", "200",
                "pdf/datacenter_sources.pdf", "png/pdf_page",
            ])
            .status();
        match rendered {
            Ok(s) if s.success() => println!("Rendered {} PDF pages", pages.len()),
            _ => {
                eprintln!("Failed to rasterize the PDF (need `pdftoppm` and pdf/datacenter_sources.pdf)");
                std::process::exit(1);
            }
        }
        let paths: Vec<&std::path::Path> = pages.iter().map(|p| std::path::Path::new(*p)).collect();
        let pdf_url = "github.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf";
        let caption = format!(
            "AI data-center buildout in five views — the full clickable PDF (every figure links to its SEC 10-K) is on GitHub:\n\n\
            1/ Power capacity (GW), operational vs. planned, with FY2025 capex & est. $/GW — Amazon, Microsoft, Google, Meta, Oracle, xAI, OpenAI, Anthropic.\n\
            2/ The SEC 10-K financials: capex FY23–25, PP&E, operating cash flow, capex÷OCF and \"leases not yet commenced\". Now incl. Oracle (RPO backlog $553B, FY26 capex ~$50B).\n\
            3/ Where the capital actually sits — compute/servers vs. real estate vs. construction-in-progress, straight from each property & equipment note.\n\
            4/ The private players (xAI, OpenAI, Anthropic): press/analyst estimates of GPUs/silicon vs. construction/power/land. It's why xAI's plant looks cheap — they cut the facility cost, but GPUs still dominate the all-in.\n\
            5/ Off-grid vs on-grid: what the 10-Ks disclose about how the data centers are powered — almost nothing. The ~56 GW behind-the-meter gas build-out is invisible to SEC filings.\n\n\
            Full PDF: {pdf_url}\n\
            #AI #DataCenters #CapEx #SEC"
        );
        let title = "AI Data-Center Capacity & SEC Financials";
        let mut ok = false;
        if !x_only {
            match linkedin::publish_images(&paths, &caption, title) {
                Ok(u) => { println!("Posted all 5 PDF pages to LinkedIn: {u}"); ok = true; }
                Err(e) => eprintln!("[linkedin] post failed: {e}"),
            }
        }
        if !li_only {
            // X allows at most 4 images per post — send the first 4 pages.
            let x_paths: Vec<&std::path::Path> = paths.iter().take(4).copied().collect();
            match twitter::publish_images(&x_paths, &caption) {
                Ok(u) => { println!("Posted first 4 PDF pages to X: {u}"); ok = true; }
                Err(e) => eprintln!("[twitter] post failed: {e}"),
            }
        }
        if !ok {
            std::process::exit(1);
        }
        return;
    }

    let post_linkedin = args.iter().any(|a| a == "--post-linkedin" || a == "--post");
    let post_twitter = args.iter().any(|a| a == "--post-twitter" || a == "--post-x");

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
    let col_w: [i32; NCOL] = [124, 150, 178, 104, 150, 300, 300];
    let table_w: i32 = col_w.iter().sum();
    let img_w = (table_w + margin * 2) as u32;

    // ---- Table content ----
    let headers: [&str; NCOL] = [
        "Company",
        "Operational\n(Up & Running)",
        "Planned / Under Construction",
        "FY2025 Capex\n(per 10-K)",
        "Est. $/GW\n(flagship)²",
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
            Cell::new("~$5–6B/GW (facility)", Style::Bold, COSTGW_FG),
            Cell::new(
                "New Carlisle, IN ($11–15B; ~2.4 GW; ~500k AWS Trainium2 — Project Rainier) · N. Virginia (~2.75 GW; $35B through 2040)",
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
            Cell::new("~$8B/GW (facility)", Style::Bold, COSTGW_FG),
            Cell::new(
                "Fairwater — Wisconsin (~$7.3B; ~0.9 GW, early 2026) · Atlanta (online; GB300 NVL72, Blackwell Ultra) · Fairwater 4 (constr.)",
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
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "Global fleet (TPU v7 Ironwood + Nvidia). $52.7B of long-term data-center leases signed but not yet commenced (10-K)",
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
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "Prometheus — New Albany, OH (~1 GW, 2026; Blackwell GB200/GB300) · Hyperion — Richland Parish, LA (→5 GW; $27B Blue Owl JV, 2,250 acres)",
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
            Cell::new("Oracle (OCI)", Style::Bold, COMPANY_FG),
            Cell::new("~2–3 GW (OCI global, est.)", Style::Regular, CELL_FG),
            Cell::new(
                ">10 GW of power secured for next 3 yrs; 4.5 GW Stargate deal with OpenAI",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$21.2B", Style::Bold, CAPEX_FG),
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "Abilene, TX (Stargate flagship; ~1.2 GW, →~450k GB200 — built by Crusoe, OCI operates) · Shackelford Co. & Doña Ana Co. · Wisconsin (Vantage) · Michigan",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "RPO backlog $553B (Q3 FY2026), mostly large AI contracts. FY2026 capex guided ~$50B. Reported ~$300B / 5-yr OpenAI compute deal. FY ends May.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("CoreWeave (CRWV)", Style::Bold, COMPANY_FG),
            Cell::new(
                "~1 GW active (→>1.7 GW by end-2026; 43 data centers)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new(
                ">3.5 GW contracted; 5+ GW w/ Nvidia by 2030 — heavily leased",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$10.31B", Style::Bold, CAPEX_FG),
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "43 leased data centers (US + Europe). First to deploy GB300 NVL72; first Vera Rubin NVL72 bring-up (Jun 2026, with Dell)",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Neocloud — rents Nvidia GPU capacity; leases its DCs, funded by ~$21B debt (9%+ notes). OpenAI ~$22.4B & Meta ~$35B contracts; Nvidia $6.3B take-or-pay backstop. Microsoft ~62% of revenue. RPO $60.7B (10-K). Net loss $1.2B. Ex-crypto miner; IPO Mar 2025.",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("xAI", Style::Bold, COMPANY_FG),
            Cell::new("~1.0 GW compute (Colossus I+II — SEC: SpaceX IPO prospectus), ~2 GW building", Style::Regular, CELL_FG),
            Cell::new(
                "Further expansions (roadmap to much larger)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("Private — n/a", Style::Regular, NOTE_FG),
            Cell::new("~$9–15B/GW (all-in)", Style::Bold, COSTGW_FG),
            Cell::new(
                "Memphis, TN — Colossus 1 (~0.3 GW; ~230k GPUs; first 100k H100 / ~130 MW in 122 days) + Colossus 2 (first 110k GB200 / ~210 MW in 91 days; ≥220k GB300 / >400 MW next); power hub in Southaven, MS",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Capacity per SpaceX IPO prospectus (8 Jun 2026) — the only press-row GW here that is SEC-disclosed. Compute leased out: Anthropic $1.25B/mo (Colossus 1, thru 2029) + Google $920M/mo for ~110k GPUs Oct 2026–Jun 2029 (FWP, 5 Jun 2026).",
                Style::Regular,
                NOTE_FG,
            ),
        ],
        [
            Cell::new("Nebius (NBIS)", Style::Bold, COMPANY_FG),
            Cell::new(
                "~0.5 GW connected (→0.8–1 GW by end-2026)",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new(
                ">3.5 GW contracted (→>4 GW by end-2026); >75% owned",
                Style::Regular,
                CELL_FG,
            ),
            Cell::new("$4.07B", Style::Bold, CAPEX_FG),
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "Owned ~3 GW / 5 sites: Independence, MO (1.2 GW) · Vineland, NJ (Microsoft) · Pennsylvania (→1.2 GW, 2027) · Alabama (2027) · Finland (310 MW). GB300 NVL72; early Vera Rubin",
                Style::Regular,
                SITE_FG,
            ),
            Cell::new(
                "Neocloud — rents Nvidia GPU capacity. Meta deal up to $27B/5 yr; Microsoft up to $19.4B thru 2031. RPO backlog $21.3B (20-F). FY ends Dec. Ex-Yandex; on Nasdaq since Oct 2024.",
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
            Cell::new("~$50B/GW (all-in)", Style::Bold, COSTGW_FG),
            Cell::new(
                "Abilene, TX (flagship → 1.2 GW; ~0.3 GW live; 450k GB200) · Shackelford Co., TX · Doña Ana Co., NM · Lordstown, OH · Wisconsin · UAE (2026)",
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
            Cell::new("— (n/d)", Style::Regular, NOTE_FG),
            Cell::new(
                "Own (Fluidstack): Abernathy, TX (~168 MW) · Lake Mariner, NY (~360 MW). Partners: AWS Trainium2 (~500k→1M), Google TPU v7 (≤1M), Azure (Nvidia), xAI Colossus 1",
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

    let footnotes: [&str; 3] = [
        "¹ Meta 10-K (FY2025): \"We anticipate making capital expenditures of approximately $115 billion to $135 billion in 2026 to support our AI efforts and core business.\"",
        "² Est. $/GW = a company's flagship-project cost ÷ that project's power. \"facility\" excludes IT (industry benchmark ~$8–12B/GW); \"all-in\" includes GPUs/servers (~$35–60B/GW; Nvidia cites $50–60B). \"n/d\" = no per-project cost disclosed. Press/analyst-derived, not an SEC figure.",
        "Capex = purchases of property & equipment from the latest annual SEC filing's cash-flow statement (Microsoft FY ends June, Oracle FY ends May; Amazon/Alphabet/Meta/CoreWeave/Nebius FY ends December). Nebius is a foreign private issuer and files Form 20-F (US GAAP), not 10-K. xAI, OpenAI & Anthropic are private and do not file with the SEC. GW capacity figures and site details are press/analyst-sourced — SEC filings do not disclose capacity in gigawatts.",
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

    if post_linkedin {
        match linkedin::publish_image(
            std::path::Path::new(out),
            &linkedin::chart_caption(),
            "Data Center Power Capacity (GW)",
        ) {
            Ok(url) => println!("Posted to LinkedIn: {url}"),
            Err(e) => {
                eprintln!("[linkedin] post failed: {e}");
                std::process::exit(1);
            }
        }
    }
    if post_twitter {
        match twitter::publish_image(std::path::Path::new(out), &twitter::chart_caption()) {
            Ok(url) => println!("Posted to X: {url}"),
            Err(e) => {
                eprintln!("[twitter] post failed: {e}");
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
