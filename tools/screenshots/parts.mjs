// Captures element-level crops of Zuno's real interface (transparent corners) and converts all
// captures to WebP for the website. node parts.mjs <baseUrl> <shotsDir> <outDir>
import { chromium } from "playwright-core";
import { readFileSync, writeFileSync, readdirSync, mkdirSync } from "node:fs";
import { join, dirname, basename } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const mock = eval(readFileSync(join(here, "mock.js"), "utf8"));
const [base, shotsDir, out] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

const parts = [
  {
    name: "part-quick",
    scenario: {},
    width: 1440,
    run: async (p) => {
      await p.evaluate(() => window.__zunoEmit("zuno://shortcut", "quick-switcher"));
      await p.keyboard.type("rust", { delay: 30 });
    },
    target: ".quick",
  },
  { name: "part-chrome", scenario: {}, width: 1680, target: ".chrome" },
  {
    name: "part-auth",
    scenario: { authOnNavigate: true },
    width: 1440,
    run: async (p) => {
      await p.getByLabel("Search or enter address").first().fill("https://accounts.example.com/oauth2/authorize");
      await p.locator(".newtab form").evaluate((f) => f.requestSubmit());
    },
    target: ".dialog",
  },
  { name: "part-incognito", scenario: { incognito: true }, width: 1440, target: ".incognito-info" },
  { name: "part-incognito-badge", scenario: { incognito: true }, width: 1440, target: ".incognito-badge" },
  {
    name: "part-privacy",
    scenario: { url: "zuno://settings", title: "Settings" },
    width: 1440,
    run: async (p) => p.getByRole("button", { name: "Privacy" }).click(),
    target: ".settings main .list >> nth=0",
  },
  { name: "part-downloads", scenario: { url: "zuno://downloads", title: "Downloads" }, width: 1440, target: ".page .list" },
  { name: "part-appearance", scenario: { url: "zuno://settings", title: "Settings" }, width: 1440, target: ".settings main .list >> nth=0" },
];

const browser = await chromium.launch({ channel: "chrome" });
for (const part of parts) {
  const ctx = await browser.newContext({ viewport: { width: part.width, height: 860 }, deviceScaleFactor: 2, colorScheme: "dark" });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.error(`[${part.name}]`, e.message));
  await page.addInitScript(mock, part.scenario);
  await page.goto(base, { waitUntil: "networkidle" });
  await page.waitForTimeout(400);
  if (part.run) await part.run(page);
  await page.waitForTimeout(400);
  await page.mouse.move(part.width - 1, 859);
  // Transparent page so rounded corners stay transparent; drop focus rings and drop shadows.
  await page.addStyleTag({
    content: `html,body,#root,.app,.content,.scrim{background:transparent!important} .scrim{animation:none!important} .quick,.dialog{box-shadow:0 0 0 1px var(--border)!important;animation:none!important} *:focus-visible{outline:none!important}`,
  });
  await page.evaluate(() => document.activeElement instanceof HTMLElement && document.activeElement.blur());
  await page.waitForTimeout(150);
  await page.locator(part.target).screenshot({ path: join(shotsDir, `${part.name}.png`), omitBackground: true });
  console.log("captured", part.name);
  await ctx.close();
}

// Convert every PNG capture to WebP (2x and 1x) using Chrome's encoder.
const ctx = await browser.newContext();
const page = await ctx.newPage();
for (const file of readdirSync(shotsDir).filter((f) => f.endsWith(".png"))) {
  const data = "data:image/png;base64," + readFileSync(join(shotsDir, file)).toString("base64");
  const res = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const enc = (scale) => {
      const c = document.createElement("canvas");
      c.width = Math.round(img.naturalWidth * scale);
      c.height = Math.round(img.naturalHeight * scale);
      const g = c.getContext("2d");
      g.imageSmoothingQuality = "high";
      g.drawImage(img, 0, 0, c.width, c.height);
      return { url: c.toDataURL("image/webp", 0.88), w: c.width, h: c.height };
    };
    return { x2: enc(1), x1: enc(0.5) };
  }, data);
  const name = basename(file, ".png");
  for (const [suffix, v] of [["@2x", res.x2], ["", res.x1]]) {
    writeFileSync(join(out, `${name}${suffix}.webp`), Buffer.from(v.url.split(",")[1], "base64"));
  }
  console.log("webp", name, res.x1.w, "x", res.x1.h);
}
await browser.close();
