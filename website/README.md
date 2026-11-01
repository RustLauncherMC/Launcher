# RustLauncherMC's website

A static site in plain HTML, CSS and JavaScript, with no build step.

| File | What it is |
| --- | --- |
| `index.html` | The page |
| `styles.css` | Styles: colours and fonts at the top, small screens at the bottom |
| `main.js` | The pixel-art sky and aurora (canvas), the launcher preview, scroll reveals, the mobile menu, and the latest release from GitHub's API |
| `fonts/` | Bricolage Grotesque and Geist, hosted here (SIL Open Font License, see the `OFL-*.txt` files) |
| `img/` | The logo |

The design follows [playhorizon.dev](https://playhorizon.dev): the same dark night palette,
fonts, pixel-art horizon and section layout, with RustLauncherMC's own content, colours and art.

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
