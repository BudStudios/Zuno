// Renders Zuno's real interface (app/dist) with mock data and captures product screenshots.
// node capture.mjs <baseUrl> <outDir> [only]
import { chromium } from "playwright-core";
import { readFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const mock = eval(readFileSync(join(here, "mock.js"), "utf8"));
const [base, out, only] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

const shots = [
  { name: "newtab", scenario: {} },
  {
    name: "quick-switcher",
    scenario: {},
    run: async (p) => {
      await p.evaluate(() => window.__zunoEmit("zuno://shortcut", "quick-switcher"));
      await p.keyboard.type("rust", { delay: 30 });
    },
  },
  {
    name: "settings-privacy",
    scenario: { url: "zuno://settings", title: "Settings" },
    run: async (p) => p.getByRole("button", { name: "Privacy" }).click(),
  },
  {
    name: "settings-appearance",
    scenario: { url: "zuno://settings", title: "Settings" },
  },
  { name: "incognito", scenario: { incognito: true } },
  {
    name: "auth-review",
    scenario: { authOnNavigate: true },
    run: async (p) => {
      await p.getByLabel("Search or enter address").first().fill("https://accounts.example.com/oauth2/authorize?client_id=notes-desktop");
      await p.locator(".newtab form").evaluate((f) => f.requestSubmit());
    },
  },
  { name: "history", scenario: { url: "zuno://history", title: "History" } },
  { name: "downloads", scenario: { url: "zuno://downloads", title: "Downloads" } },
  { name: "bookmarks", scenario: { url: "zuno://bookmarks", title: "Bookmarks" } },
  { name: "import", scenario: { url: "zuno://import", title: "Import" } },
  { name: "light", scenario: { theme: "light" } },
  {
    name: "light-settings",
    scenario: { theme: "light", url: "zuno://settings", title: "Settings" },
    run: async (p) => p.getByRole("button", { name: "Privacy" }).click(),
  },
];

const browser = await chromium.launch({ channel: "chrome" });
const light = !!process.env.LIGHT;
for (const shot of shots) {
  if (light) { if (shot.name.startsWith("light")) continue; shot.scenario = { ...shot.scenario, theme: "light" }; shot.name += "-light"; }
  if (only && !only.split(",").includes(shot.name)) continue;
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 2, colorScheme: shot.scenario.theme ?? "dark" });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.error(`[${shot.name}] pageerror`, e.message));
  page.on("console", (m) => m.type() === "error" && console.error(`[${shot.name}] console`, m.text()));
  await page.addInitScript(mock, shot.scenario);
  await page.goto(base, { waitUntil: "networkidle" });
  await page.waitForTimeout(400);
  if (shot.run) await shot.run(page);
  await page.waitForTimeout(500);
  // Park the pointer off-canvas so no hover state is captured.
  await page.mouse.move(1439, 859);
  await page.waitForTimeout(150);
  await page.screenshot({ path: join(out, `${shot.name}.png`) });
  console.log("captured", shot.name);
  await ctx.close();
}
await browser.close();
