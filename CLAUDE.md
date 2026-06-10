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
cargo run --release --bin spacex_exposure     # -> pdf/spacex_exposure.pdf
cargo build --release                         # build all binaries

# LinkedIn (datacenter_chart only):
cargo run --release --bin datacenter_chart -- --auth           # OAuth, writes linkedin_token.json
cargo run --release --bin datacenter_chart -- --post-linkedin  # render PNG, then post it

# X/Twitter (datacenter_chart only):
cargo run --release --bin datacenter_chart -- --post-twitter   # alias --post-x; flags compose with --post-linkedin
cargo run --release --bin datacenter_chart -- --post-sec       # render PDF page 2 -> png/sec_financials.png, post to both
cargo run --release --bin datacenter_chart -- --post-pdf       # rasterize all 5 PDF pages -> png/pdf_page-{1,2,3,4,5}.png, post as ONE multi-image post to LinkedIn (all 5) + X (first 4)
cargo run --release --bin datacenter_chart -- --post-pdf-li    # same, LinkedIn only
cargo run --release --bin datacenter_chart -- --post-pdf-x     # same, X only (multi-image)
cargo run --release --bin datacenter_chart -- --post-pdf-thread # post the 5 pages to X as a reply-chain thread
cargo run --release --bin datacenter_chart -- --post-pdf-doc   # post pdf/datacenter_sources.pdf to LinkedIn as a NATIVE document (Documents API)
cargo run --release --bin datacenter_chart -- --post-spacex-doc # post pdf/spacex_exposure.pdf to LinkedIn as a NATIVE document (German caption)
cargo run --release --bin datacenter_chart -- --post-png <path> <caption>  # post one PNG as a plain standalone tweet
cargo run --release --bin datacenter_chart -- --delete-tweet <id>

# Signal (datacenter_chart only; build needs PROTOC=~/.local/protoc/bin/protoc — system protoc 3.6.1 is too old for libsignal):
cargo run --release --bin datacenter_chart -- --signal-link        # once: provisioning QR -> /tmp/signal_link_qr.png, opened in viewer, scan with phone
cargo run --release --bin datacenter_chart -- --signal-groups      # list groups: <64-hex master key>  <title>
cargo run --release --bin datacenter_chart -- --post-signal <group> [message]  # send pdf/datacenter_sources.pdf to the group
cargo run --release --bin datacenter_chart -- --signal-messages <group>        # dump stored thread (timestamps, attachment counts)
cargo run --release --bin datacenter_chart -- --signal-delete <group> <ts>     # delete-for-everyone (ts printed by --post-signal)
```

`--post-pdf` posts to LinkedIn (`multiImage`) and best-effort to X; it still
succeeds if LinkedIn posts even when X fails. `--post-pdf-li`/`--post-pdf-x`
restrict to one network; `--post-pdf-thread` posts the 5 pages to X as a
reply-chain (one single-image tweet per page). `--post-pdf` sends all 5 pages
to LinkedIn but only the first 4 to X (X caps multi-image posts at 4).

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

Three independent binaries (declared in `Cargo.toml`); there is **no shared
library** — the PNG/PDF data-center dataset is duplicated in `datacenter_chart`
and `datacenter_pdf`, so a content change must be applied in both files. The
`spacex_exposure` binary is standalone (its own dataset; it copies the same
`printpdf` `Pdf` helper struct rather than sharing it):

- `src/main.rs` (`datacenter_chart`) — renders the PNG with the `image` +
  `ab_glyph` crates. Hand-rolled table layout: a `rows: Vec<[Cell; NCOL]>`
  drives fixed-width columns; `wrap_text` reflows cell text; row heights derive
  from the tallest wrapped cell; glyphs are rasterized with alpha blending.
- `src/bin/datacenter_pdf.rs` (`datacenter_pdf`) — renders the **5-page** PDF
  with `printpdf` 0.9 (an `Op`-based document model). Filled header/row
  rectangles (`Op::DrawPolygon` with `PaintMode::Fill` — note
  `Op::DrawRectangle` does **not** fill in 0.9), grid lines, and per-row source
  hyperlinks (`Op::LinkAnnotation` with `Actions::Uri`). Page 1: the capacity
  table (`rows: [Row; _]`). Page 2: the SEC-financials table (`[Sec; _]`) with
  its footnotes. Page 3: the **PP&E-composition** table (compute/servers vs.
  real estate vs. construction-in-progress vs. finance-lease ROU, FY2025 gross
  per filing) with its footnotes. (Pages 2+3 were one page until the footnotes
  were word-wrapped — wrapped, they no longer fit together, so the SEC table and
  the composition table each got their own page.) Note **Nebius** (NBIS) is a
  foreign private issuer: it files Form **20-F** (US GAAP), not a 10-K, so its
  source links read "20-F ↗" — the `Sec` struct carries a `form` field for the
  SEC table, and the PP&E-composition label switches on `co.starts_with("Nebius")`.
  Page 4: **private operators** (xAI/OpenAI/Anthropic) GPU-vs-plant *estimates*
  — press/analyst, not SEC, with the **SEC exceptions** footnoted: xAI's compute is
  sold via SpaceX, whose IPO Free Writing Prospectus (`SPACEX_FWP`, Rule 433, File
  333-296070, filed 5 Jun 2026) discloses a Google Cloud Service Agreement —
  $920M/mo for ~110k Nvidia GPUs, Oct 2026–Jun 2029 — and whose second FWP
  (`SPACEX_EU_FWP`, 8 Jun 2026, attaching the BaFin-approved EU retail prospectus)
  discloses **Colossus I+II ≈1.0 GW of compute power** (C1 first cluster ~100k
  H100/~130 MW in 122 days; C2 ~110k GB200/~210 MW in 91 days; ≥220k GB300/
  >400 MW next) — the first GW figure for xAI in an SEC-filed document. The xAI
  page-1 row carries a second `FWP ↗` link (like Alphabet's second FWP link), its
  operational cell cites the prospectus ~1.0 GW, and its Key Notes name both the
  Google and Anthropic ($1.25B/mo) compute leases. Page 5: **off-grid vs on-grid CAPACITY** — an
  `og: [(operator, off_grid_text, on_grid_text, on_grid_is_sec); 10]` table with
  an amber off-grid/behind-the-meter capacity column (press/permit/satellite,
  NOT in SEC) and an on-grid column shown **green only when the figure is
  actually SEC-disclosed** (CoreWeave's 10-K & Nebius's 20-F active/contracted
  GW, plus xAI's ~1.0 GW via the SpaceX prospectus — flagged "off-grid by
  design"; everything else is analyst estimate). The footnotes give the
  capacity split (Cleanview: ~56 GW planned off-grid ≈ 30% of the US pipeline,
  ~2 GW online, mostly gas; the other ~70% grid) and note that NO filer disclose
  any off-grid capacity or grid split (SpaceX's prospectus included), with a
  linked SpaceX-FWP source line under the Cleanview link. The composition, private and off-grid tables are drawn
  by the reusable `Pdf::draw_table` helper (header band + alternating rows + grid; each
  cell is `(text, bold, color, Option<url>)`). **All subtitles and footnotes use
  the word-wrapping `Pdf::paragraph` (which returns the running `y`, so blocks
  flow down the page); plain `Pdf::line` does NOT wrap and overflows the right
  edge — only use it for short single-line titles.** `--post-sec` still
  rasterizes **page 2** only (still the SEC-financials table).
- `src/bin/spacex_exposure.rs` (`spacex_exposure`) — renders a **1-page** A4
  PDF (`pdf/spacex_exposure.pdf`) of publicly-accessible funds that hold SpaceX
  equity, each row linking to the SEC filing (N-PORT / N-CSR) that discloses the
  stake. It copies the same `Pdf` helper struct as `datacenter_pdf` (note: its
  link-underline width is multiplied by `WRAP_FUDGE` so the underline spans the
  full rendered label — `ab_glyph` under-measures poppler ~12%; the original
  `datacenter_pdf` does **not** do this, fine for its short labels). A `Holder`
  struct drives the table; columns are **SpaceX now (USD)** (sum of all SpaceX
  lines in the filing — common share classes + preferred series + SPVs) and
  **Value at est. IPO** (that mark × an illustrative 1.2×–1.6×, i.e. a $1.5–2T
  IPO over the ~$1.25T combined SpaceX+xAI mark; the reported $1.77T pricing is
  ~1.42×). Holdings figures were located via SEC EDGAR full-text search
  (efts.sec.gov) for the exact phrase "Space Exploration Technologies" and each
  filing line verified; USD values are summed per fund. The two NYSE-listed
  funds (DXYZ, BCAT) show "— listed" in the IPO column (already market-priced).
  Posted to LinkedIn as a native document via `--post-spacex-doc` (German
  caption hard-coded in `main.rs`) and to X as a rasterized PNG
  (`png/spacex_exposure.png`) via `--post-png`.
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
  with an SEC caption. `--post-pdf` (in `main.rs`) rasterizes all 5 PDF pages →
  `png/pdf_page-{1,2,3,4,5}.png` and posts them as ONE multi-image post (all 5
  to LinkedIn, first 4 to X — X caps at 4) via
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
- `src/signal.rs` — Signal group messaging via **presage** (Rust client stack
  on the official libsignal crates; git deps pinned by rev in `Cargo.toml`,
  which also repeats presage's required `[patch.crates-io]` entries — patches
  only apply at the workspace root). The machine is a **linked secondary
  device** ("gigacrawl") of the user's Signal account; state in
  `signal_store.db3` (cwd/$HOME, gitignored — it can send as the user). presage
  is async: each entry point wraps a current-thread tokio runtime + `LocalSet`
  (`block_on`). `link_device()` renders the provisioning QR to
  `/tmp/signal_link_qr.png` and `open::that`s it (terminal QR art mangles in
  some emulators); the provisioning socket is short-lived — on "no provisioning
  message received", rerun and scan immediately. Every send/list first drains
  the incoming queue (`sync_until_empty`) because **groups/contacts are only
  learned from synced messages** — an unknown group means nobody has posted in
  it since linking. `send_pdf_to_group` uploads via
  `manager.upload_attachments` then sends a `DataMessage` with `group_v2`
  context (revision 0 is fine) and prints the sent **timestamp**, which
  `delete_group_message` (`--signal-delete`) needs for delete-for-everyone —
  Signal edits can't add attachments, so fixing a botched post = delete +
  resend. The sender's own phone displays the synced sent-copy and may not
  render the attachment chip even when `--signal-messages` shows
  `attachments=1` for the stored message. Build gotchas: needs protoc ≥ 3.12
  (`PROTOC=~/.local/protoc/bin/protoc`, official 25.3 binary installed there;
  system Gentoo protoc is 3.6.1) — and beware `pkill -f <pattern>` in this
  sandbox: it matches its own wrapper shell and kills the command chain (exit
  144); use `pkill -x`.

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
