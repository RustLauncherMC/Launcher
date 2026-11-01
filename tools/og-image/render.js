// Renders og.html to website/og.png (1200 × 630), the image social sites show for links to the
// website. Needs Node and Playwright with Chromium:
//
//   npm install --no-save playwright && npx playwright install chromium
//   node tools/og-image/render.js
const path = require("path");
const { chromium } = require("playwright");

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1200, height: 630 } });
  await page.goto("file://" + path.join(__dirname, "og.html"));
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(300);
  const out = path.join(__dirname, "..", "..", "website", "og.png");
  await page.screenshot({ path: out });
  await browser.close();
  console.log("Wrote", out);
})();
