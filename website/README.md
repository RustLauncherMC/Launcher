# RustLauncherMC's website

A static site in plain HTML, CSS and JavaScript, with no build step.

| File | What it is |
| --- | --- |
| `index.html` | The page |
| `styles.css` | Styles: colours and fonts at the top, small screens at the bottom |
| `main.js` | The pixel cog and embers (canvas), the launcher preview, the navbar (shrinks on scroll, highlights the section in view, mobile menu), scroll reveals, and the latest release from GitHub's API |
| `fonts/` | Bricolage Grotesque and Geist as WOFF2, cut down to Latin characters with every weight kept (SIL Open Font License, see the `OFL-*.txt` files) |
| `og.png` | The 1200 × 630 image social sites show for links, made from `tools/og-image/og.html` |
| `robots.txt`, `sitemap.xml`, `site.webmanifest`, `404.html` | For search engines, browsers, and missing pages on GitHub Pages |
| `img/` | The logo. The isometric block is also inlined in `index.html` as an SVG symbol |

## Brand

The colours come from the logo and the launcher itself:

| Colour | Where it comes from | Used for |
| --- | --- | --- |
| Charcoal `#131416` to `#2a2c31` | The logo's tile, the launcher's window | Background and surfaces |
| Rust orange `#f0795a` to `#b7410e` | The logo's cog | Main buttons, highlights, the cog behind the hero |
| Grass green `#6db33f` | The block's top | "New" tag, list markers |
| Java purple `#3b2f7e`, Bedrock red `#b22222` | The block's sides | Each edition's colour |
| Play green `#3c8527` | The launcher's Play button | Only in the launcher preview |

The headings use Bricolage Grotesque and the text uses Geist. The section layout started from
[playhorizon.dev](https://playhorizon.dev)'s.

## Try it locally

```sh
cd website
python3 -m http.server 8000
```

Then open http://localhost:8000.

## Publishing

`.github/workflows/pages.yml` publishes this folder to GitHub Pages on every change to `main`.
Turn it on once in the repository's **Settings → Pages → Build and deployment → Source:
GitHub Actions**. The site is then at `https://rustlaunchermc.github.io/Launcher/`, or on a custom
domain set in the same settings page.

## SEO

- **Title and description:** under 60 and 160 characters, so search results show them whole.
- **Canonical URL:** `https://rustlaunchermc.github.io/Launcher/`.
- **Social previews:** Open Graph and Twitter/X tags with `og.png`.
- **Structured data:** JSON-LD in `index.html` describes the site (`WebSite`), the project
  (`Organization`), the launcher (`SoftwareApplication`: free, Windows and Linux, MIT) and the
  FAQ (`FAQPage`). The FAQ answers must stay word for word the same as the FAQ section, so
  update both together.
- **Crawling:** `robots.txt` allows everything and points to `sitemap.xml`. The 404 page asks
  not to be indexed.
- **Speed:** WOFF2 fonts (185 KB instead of 577 KB), preloaded, and a preconnect to GitHub's API.

**On a custom domain**, replace `https://rustlaunchermc.github.io/Launcher/` in `index.html`
(canonical, Open Graph, Twitter and JSON-LD), `robots.txt` and `sitemap.xml`, and the `/Launcher/`
paths in `404.html`.

### The social image

`og.png` is rendered from `tools/og-image/og.html`, which uses the site's fonts and logo. After
changing it:

```sh
npm install --no-save playwright && npx playwright install chromium
node tools/og-image/render.js
```

To check a preview, paste the URL into a link-preview tool such as
[opengraph.xyz](https://www.opengraph.xyz/), or post it in a private chat.

## Downloads

The download buttons read the latest release from
`https://api.github.com/repos/RustLauncherMC/Launcher/releases/latest` and link straight to the
file for the visitor's system. Before the first release, they lead to the Download section and
the page says a release is on its way. Asset names must match `.github/workflows/release.yml`.
