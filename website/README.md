# RustLauncherMC's website

A static site in plain HTML, CSS and JavaScript, with no build step.

| File | What it is |
| --- | --- |
| `index.html` | The page |
| `styles.css` | Styles: colours and fonts at the top, small screens at the bottom |
| `main.js` | The pixel cog and embers (canvas), the launcher preview, the navbar (shrinks on scroll, highlights the section in view, mobile menu), scroll reveals, and the latest release from GitHub's API |
| `fonts/` | Bricolage Grotesque and Geist, hosted here (SIL Open Font License, see the `OFL-*.txt` files) |
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

## Downloads

The download buttons read the latest release from
`https://api.github.com/repos/RustLauncherMC/Launcher/releases/latest` and link straight to the
file for the visitor's system. Before the first release, they lead to the Download section and
the page says a release is on its way. Asset names must match `.github/workflows/release.yml`.
