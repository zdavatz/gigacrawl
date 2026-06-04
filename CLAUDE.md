# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**gigacrawl** — Compare data centers worldwide.

A Rust project that renders a comparison of major AI / cloud data-center
operators' power capacity (GW) — operational vs. planned — with FY2025 capex
figures sourced from SEC 10-K filings. Two outputs: a PNG chart and a linked PDF table.

## Commands

```sh
cargo run --release --bin datacenter_chart   # -> png/datacenter_capacity.png
cargo run --release --bin datacenter_pdf      # -> pdf/datacenter_sources.pdf
cargo build --release                         # build both binaries

# LinkedIn (datacenter_chart only):
cargo run --release --bin datacenter_chart -- --auth           # OAuth, writes linkedin_token.json
cargo run --release --bin datacenter_chart -- --post-linkedin  # render PNG, then post it

# X/Twitter (datacenter_chart only):
cargo run --release --bin datacenter_chart -- --post-twitter   # alias --post-x; flags compose with --post-linkedin
cargo run --release --bin datacenter_chart -- --post-sec       # render PDF page 2 -> png/sec_financials.png, post to both
cargo run --release --bin datacenter_chart -- --post-pdf       # rasterize all 3 PDF pages -> png/pdf_page-{1,2,3}.png, post as ONE multi-image post to LinkedIn + X
cargo run --release --bin datacenter_chart -- --post-pdf-li    # same, LinkedIn only
cargo run --release --bin datacenter_chart -- --post-pdf-x     # same, X only (multi-image)
cargo run --release --bin datacenter_chart -- --post-pdf-thread # post the 3 pages to X as a reply-chain thread
cargo run --release --bin datacenter_chart -- --post-pdf-doc   # post pdf/datacenter_sources.pdf to LinkedIn as a NATIVE document (Documents API)
cargo run --release --bin datacenter_chart -- --post-png <path> <caption>  # post one PNG as a plain standalone tweet
cargo run --release --bin datacenter_chart -- --delete-tweet <id>
```

`--post-pdf` posts to LinkedIn (`multiImage`) and best-effort to X; it still
succeeds if LinkedIn posts even when X fails. `--post-pdf-li`/`--post-pdf-x`
restrict to one network; `--post-pdf-thread` posts the 3 pages to X as a
reply-chain (one single-image tweet per page).

**X pay-per-use posting (mid-2026)** — `POST /2/media/upload` always succeeds,
but `POST /2/tweets` (the *tweet create* call) `403`s ("not permitted") in two
cases: (a) **multi-image** posts and **reply** tweets (threads) are blocked
outright on this tier; (b) even plain single-image creates succeed only a few
times per short window before further ones `403` — a write **rate-limit**
returned as `403`, not `429`. Reliable X path = spaced-out single-image posts
(`--post-png` / `--post-twitter`). Not a creds/code problem.

`--post-pdf-doc` is the only way to get the *actual PDF* onto a network:
LinkedIn renders it as a swipeable, downloadable carousel, but its in-feed
viewer rasterizes pages so the 10-K hyperlinks are clickable only after
download. X accepts no PDFs.

There is no test suite. Verify changes by rendering and inspecting the output
(e.g. `feh png/datacenter_capacity.png`, or rasterize the PDF with
`pdftoppm -png -r 150 pdf/datacenter_sources.pdf /tmp/out` and view).

## Architecture

Two independent binaries (declared in `Cargo.toml`); there is **no shared
library** — the dataset is duplicated in each, so a content change must be
applied in both files:

- `src/main.rs` (`datacenter_chart`) — renders the PNG with the `image` +
  `ab_glyph` crates. Hand-rolled table layout: a `rows: Vec<[Cell; NCOL]>`
  drives fixed-width columns; `wrap_text` reflows cell text; row heights derive
  from the tallest wrapped cell; glyphs are rasterized with alpha blending.
- `src/bin/datacenter_pdf.rs` (`datacenter_pdf`) — renders the **3-page** PDF
  with `printpdf` 0.9 (an `Op`-based document model). Filled header/row
  rectangles (`Op::DrawPolygon` with `PaintMode::Fill` — note
  `Op::DrawRectangle` does **not** fill in 0.9), grid lines, and per-row source
  hyperlinks (`Op::LinkAnnotation` with `Actions::Uri`). Page 1: the capacity
  table (`rows: [Row; _]`). Page 2: the SEC-financials table (`[Sec; _]`) plus a
  **PP&E-composition** table (compute/servers vs. real estate vs.
  construction-in-progress vs. finance-lease ROU, FY2025 gross per filing). Note
  **Nebius** (NBIS) is a foreign private issuer: it files Form **20-F** (US GAAP),
  not a 10-K, so its source links read "20-F ↗" — the `Sec` struct carries a
  `form` field for the SEC table, and the PP&E-composition label switches on
  `co.starts_with("Nebius")`. Page 3:
  **private operators** (xAI/OpenAI/Anthropic) GPU-vs-plant *estimates* — press/
  analyst, not SEC. The two later tables are drawn by the reusable
  `Pdf::draw_table` helper (header band + alternating rows + grid; each cell is
  `(text, bold, color, Option<url>)`); long footnotes use `Pdf::paragraph`
  (word-wrapped — plain `Pdf::line` does **not** wrap and will overflow the page).
  `--post-sec` still rasterizes **page 2** only.
- `src/linkedin.rs` — LinkedIn OAuth + image publishing, used only by
  `datacenter_chart` (via `mod linkedin;`). Self-contained: blocking `reqwest`
  + a `std::net::TcpListener` callback server on port 8092 (no tokio). Uses
  gigacrawl's **own** app credentials (`linkedin_credentials.json` /
  `linkedin_token.json`, looked up in cwd then `$HOME`, both gitignored) so it
  does not share/rotate tokens with sibling projects. The token rotates on
  refresh and is persisted back. `--auth` runs the authorization-code flow;
  `--post-linkedin` posts `png/datacenter_capacity.png` (Images API →
  Posts API). The caption lives in `caption()` and must be passed through
  `escape_little_text` (LinkedIn truncates on unescaped control chars).
  `publish_images(paths, commentary, title)` posts several PNGs as one post:
  `content.media` for a single image, `content.multiImage` (2–20 images, each
  with `altText`) for more; `publish_image` delegates to it. The shared
  `upload_image` helper does initializeUpload → PUT. `publish_document(path,
  commentary, title)` posts a **native PDF document** via the parallel Documents
  API (`/rest/documents?action=initializeUpload` → PUT → Posts API with
  `content.media` = the `urn:li:document:…`), via `upload_document` +
  `create_document_post`. The Documents API uses the same `w_member_social`
  scope as images and is confirmed working for gigacrawl's app.
- `src/twitter.rs` — X/Twitter image posting, used only by `datacenter_chart`
  (`--post-twitter` / `--post-x`). Hand-rolled OAuth 1.0a (HMAC-SHA1; only the
  oauth_* params are signed — multipart and JSON bodies are excluded, which is
  correct for both endpoints). Uploads via v2 `/2/media/upload` (the v1.1
  endpoint was retired 2025-03-31; OAuth 2.0 tokens are not accepted for media)
  with the required `media_category=tweet_image`, then tweets via v2
  `/2/tweets`. Reads `twitter_credentials.json` (cwd/$HOME) or falls back to
  parsing `~/.twurlrc`. `publish_image(path, caption)` takes the caption;
  `chart_caption()` is the default (links directly to the PDF on GitHub; X has no
  PDF attachment). `--post-sec` (in `main.rs`) shells out to `pdftoppm` to
  rasterize PDF page 2 → `png/sec_financials.png` and posts it to both networks
  with an SEC caption. `--post-pdf` (in `main.rs`) rasterizes all 3 PDF pages →
  `png/pdf_page-{1,2,3}.png` and posts them as ONE multi-image post via
  `twitter::publish_images(paths, caption)` (shared `upload_media` +
  `create_tweet` helpers; up to 4 images) and `linkedin::publish_images`.
  `create_tweet` takes an optional `reply_to` tweet id and returns the new
  tweet's **id** (callers format the URL); `publish_thread(items)` builds a
  reply-chain (each tweet replies to the previous) and backs `--post-pdf-thread`.
  `publish_image(path, caption)` posts a single standalone tweet and backs
  `--post-png`. `linkedin::publish_image(path, commentary, title)` is likewise
  caption-parameterized. Gotcha: all four OAuth values must come from
  the **same** app and the app must be Read+Write — a `401`/`code 89` means
  mismatched/invalid creds; `403` means the account lacks write/credit (X free
  tier ended Feb 2026, writes are pay-per-use).

### Conventions that matter

- **Fonts** are embedded via `include_bytes!("/usr/share/fonts/dejavu/...")`.
  Both binaries fail to compile if DejaVu Sans is absent at that path.
- **Row order** in both binaries is by estimated operational GW, descending.
- **Links are deduplicated**: exactly one source URL per row (in the Capex
  column). Public companies → their 10-K; private → a press source. Alphabet
  carries a second, *distinct* document link (the FWP for its $80B equity raise).
- **PDF coordinates** are points from the bottom-left; helpers convert from a
  top-down baseline (`by = PAGE_H - top`).
- **Text wrapping in the PDF** uses `WRAP_FUDGE` (~1.14) because poppler renders
  the embedded subset font ~10% wider than `ab_glyph` measures; without it,
  cell text overflows the right border.

## Data sources

Financial figures (capex, PP&E, lease commitments, guidance) come from FY2025
10-Ks via SEC EDGAR (`data.sec.gov`) — except **Nebius**, a foreign private
issuer whose figures come from its FY2025 Form **20-F** (US GAAP). The filing
URLs are hard-coded in `datacenter_pdf.rs`. Gigawatt capacities and site
locations are press/analyst-sourced — SEC filings do not state capacity in
gigawatts. Note **Nebius** and **CoreWeave** are **neoclouds** (they rent Nvidia
GPU capacity rather than operating purely for itself); their page-1 "operational"
figure is connected/active power, with contracted power (>3.5 GW each) shown
under "planned". CoreWeave (CRWV) is a domestic 10-K filer (so no `form`
special-casing) but is the **leased/leveraged** contrast to Nebius's owned model:
it leases its data centers (operating-lease ROU $8.23B) and is financed by ~$21.4B
of debt rather than equity — its leverage is in borrowings, not GAAP finance
leases ($0.44B) — so its capex÷OCF (337%) is far lower than Nebius's (1057%)
because it generates real operating cash flow yet still runs a net loss on
interest expense (footnote ⁶ on page 2).
