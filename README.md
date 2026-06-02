# gigacrawl

Compare data centers worldwide.

This repository contains Rust generators that render a comparison of major AI /
cloud data-center operators' **power capacity (GW)** — operational vs. planned —
alongside **FY2025 capital expenditure** figures pulled from SEC 10-K filings.

## Outputs

- [`png/datacenter_capacity.png`](png/datacenter_capacity.png) — the chart as a
  styled table (title, header band, alternating rows, wrapped cells).
- [`pdf/datacenter_sources.pdf`](pdf/datacenter_sources.pdf) — the same table in
  A4 landscape, where each row's **Capex** cell carries a clickable source link:
  public companies link to their FY2025 **10-K on sec.gov**; private companies
  (xAI, OpenAI, Anthropic) link to their primary public announcement.

Covered: Amazon (AWS), Microsoft (Azure), Google (Cloud), Meta, xAI, OpenAI,
Anthropic — ordered by estimated operational GW.

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
