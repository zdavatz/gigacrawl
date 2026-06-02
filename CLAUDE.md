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
cargo run --release --bin datacenter_chart -- --delete-tweet <id>
```

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
- `src/bin/datacenter_pdf.rs` (`datacenter_pdf`) — renders the PDF with
  `printpdf` 0.9 (an `Op`-based document model). A `rows: [Row; _]` table with
  filled header/row rectangles (`Op::DrawPolygon` with `PaintMode::Fill` —
  note `Op::DrawRectangle` does **not** fill in 0.9), grid lines, and per-row
  source hyperlinks (`Op::LinkAnnotation` with `Actions::Uri`).
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
  with an SEC caption. `linkedin::publish_image(path, commentary, title)` is
  likewise caption-parameterized. Gotcha: all four OAuth values must come from
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
10-Ks via SEC EDGAR (`data.sec.gov`); the 10-K URLs are hard-coded in
`datacenter_pdf.rs`. Gigawatt capacities and site locations are
press/analyst-sourced — SEC filings do not state capacity in gigawatts.
