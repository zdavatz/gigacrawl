# gigacrawl

Compare data centers worldwide.

This repository contains Rust generators that render a comparison of major AI /
cloud data-center operators' **power capacity (GW)** — operational vs. planned —
alongside **FY2025 capital expenditure** figures pulled from SEC 10-K filings.

## Outputs

- [`png/datacenter_capacity.png`](png/datacenter_capacity.png) — the chart as a
  styled table (title, header band, alternating rows, wrapped cells).
- [`pdf/datacenter_sources.pdf`](pdf/datacenter_sources.pdf) — **3 pages**, A4
  landscape: (1) the same table, where each row's **Capex** cell carries a
  clickable source link (public → FY2025 **10-K on sec.gov**; private → primary
  announcement); (2) a **SEC financials** page (capex FY23–25, PP&E, operating
  cash flow, capex÷OCF, leases-not-yet-commenced) plus a **PP&E composition**
  table splitting audited PP&E into compute/servers vs. real estate vs.
  construction-in-progress vs. finance-lease ROU, each row linked to its 10-K;
  (3) a **private operators** page with press/analyst estimates of the
  GPU/silicon vs. construction/power/land split for xAI, OpenAI and Anthropic
  (clearly separated from the SEC pages, as none of the three file with the SEC).
- `png/sec_financials.png` — page 2 rasterized for social posting (generated on
  demand by `--post-sec`).

Covered: Amazon (AWS), Microsoft (Azure), Google (Cloud), Meta, Oracle (OCI),
xAI, OpenAI, Anthropic — ordered by estimated operational GW.

## Build & run

Requires a Rust toolchain and the DejaVu Sans fonts (the binaries
`include_bytes!` them from `/usr/share/fonts/dejavu/`).

```sh
# Generate the PNG chart -> png/datacenter_capacity.png
cargo run --release --bin datacenter_chart

# Generate the linked PDF table -> pdf/datacenter_sources.pdf
cargo run --release --bin datacenter_pdf
```

## Post the chart to LinkedIn

`datacenter_chart` can publish the rendered PNG to LinkedIn. It uses **its own**
LinkedIn app credentials (kept separate from other projects so tokens don't
collide), read from `linkedin_credentials.json` / `linkedin_token.json` in the
current directory or `$HOME` (both gitignored).

One-time setup:

1. Create a LinkedIn app at <https://www.linkedin.com/developers/>. Add the
   products **"Sign In with LinkedIn using OpenID Connect"** and
   **"Share on LinkedIn"**, and add the redirect URL
   `http://localhost:8092/callback`.
2. Save the app keys to `linkedin_credentials.json`:
   ```json
   {"client_id": "...", "client_secret": "..."}
   ```
3. Authorize (opens a browser, writes `linkedin_token.json`):
   ```sh
   cargo run --release --bin datacenter_chart -- --auth
   ```

Then render **and** post in one step:

```sh
cargo run --release --bin datacenter_chart -- --post-linkedin
```

## Post the chart to X (Twitter)

`datacenter_chart --post-twitter` (alias `--post-x`) uploads the PNG via the
v2 `/2/media/upload` endpoint and tweets it via v2 `/2/tweets`, signed with
**OAuth 1.0a** (image upload requires OAuth 1.0a — OAuth 2.0 tokens are not
accepted). Credentials come from `twitter_credentials.json` (cwd or `$HOME`):

```json
{"consumer_key":"...","consumer_secret":"...","token":"...","secret":"..."}
```

If that file is absent it falls back to the first profile in `~/.twurlrc`. All
four values must come from the **same** app, and the app's **User
authentication settings** must be **Read and write** (regenerate the Access
Token *after* enabling write).

```sh
cargo run --release --bin datacenter_chart -- --post-twitter
# flags compose: --post-linkedin --post-twitter posts to both
```

### Post the SEC financials page

`--post-sec` rasterizes **page 2** of `pdf/datacenter_sources.pdf` (the SEC
10-K financials table) to `png/sec_financials.png` (via `pdftoppm`) and posts it
to **both** LinkedIn and X with an SEC-specific caption:

```sh
cargo run --release --bin datacenter_chart -- --post-sec   # needs pdftoppm + the PDF
```

### Post all three PDF pages

`--post-pdf` rasterizes **all three** PDF pages (`pdftoppm -r 200` →
`png/pdf_page-{1,2,3}.png`) and publishes them as a **single multi-image post**
to **LinkedIn** (`multiImage`) and, best-effort, to **X** (up to 4 images). The
caption always links to the full clickable PDF on GitHub. Network-restricted
variants:

```sh
cargo run --release --bin datacenter_chart -- --post-pdf        # LinkedIn + X
cargo run --release --bin datacenter_chart -- --post-pdf-li     # LinkedIn only
cargo run --release --bin datacenter_chart -- --post-pdf-x      # X only (multi-image)
cargo run --release --bin datacenter_chart -- --post-pdf-thread # X reply-chain thread
```

### Post the PDF itself to LinkedIn (native document)

`--post-pdf-doc` uploads `pdf/datacenter_sources.pdf` to LinkedIn as a **native
document post** (Documents API → `content.media`), which renders in-feed as a
swipeable, downloadable carousel rather than flat images:

```sh
cargo run --release --bin datacenter_chart -- --post-pdf-doc    # LinkedIn only
```

Note: LinkedIn's in-feed PDF viewer **rasterizes** the pages, so the per-figure
10-K hyperlinks are clickable only after a reader **downloads** the PDF — the
caption points at the GitHub copy for click-through. X accepts no PDFs.

### Post a single PNG to X

`--post-png <path> <caption>` posts one PNG as a plain standalone tweet —
general-purpose, and the only tweet form that reliably clears X pay-per-use
(see below).

```sh
cargo run --release --bin datacenter_chart -- --post-png png/pdf_page-2.png "caption text"
```

Notes on X posting:
- X discontinued the free API tier in Feb 2026 — posting is pay-per-use (needs
  API credit) or a legacy paid plan.
- A `401`/`code 89` means the OAuth credentials are invalid or the four values
  are from different apps; a `403` means the account/plan lacks write or credit.
- **Pay-per-use `POST /2/tweets` 403:** as of mid-2026 X pay-per-use accounts
  hit `403 "You are not permitted to perform this action"` on the *tweet create*
  call in two situations, while `POST /2/media/upload` always succeeds:
  (a) **multi-image** posts and **reply** tweets (threads) appear to be blocked
  outright on this tier; (b) even plain single-image posts succeed only a few
  times before further creates 403 — consistent with a short-window **write
  rate-limit** returned as `403` rather than `429`. `--post-pdf` therefore treats
  X as best-effort and still succeeds if LinkedIn posts; for X, plain
  single-image `--post-png`/`--post-twitter` (spaced out) is the reliable path.
- X accepts only **images/video** — not PDFs. The PDF lives in the repo and is
  referenced by link in the post text.

## Data sources & caveats

- **Capex / PP&E** come from each company's latest annual **10-K** (via SEC
  EDGAR, `data.sec.gov`). Microsoft's fiscal year ends in June; the others in
  December. Alphabet and Meta report PP&E including finance-lease right-of-use
  assets.
- **Gigawatt capacity figures and site locations** are press/analyst-sourced —
  SEC filings do **not** disclose data-center capacity in gigawatts.
- Figures are estimates as of mid-2026 and will change.

## License

GPL-3.0 (see [`LICENSE`](LICENSE)).
