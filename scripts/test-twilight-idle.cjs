const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium, devices } = require("playwright");
const base = process.env.TEST_ORIGIN || "http://127.0.0.1:1438";
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/twilight-idle");
fs.mkdirSync(out, { recursive: true });

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  try {
    for (const [name, device] of [
      ["desktop", { viewport: { width: 1500, height: 800 } }],
      ["iphone", devices["iPhone 13"]],
    ]) {
      const context = await browser.newContext(device);
      const page = await context.newPage(),
        errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.addInitScript(() => {
        window.__idleDraws = [];
        const draw = CanvasRenderingContext2D.prototype.drawImage;
        CanvasRenderingContext2D.prototype.drawImage = function (image, ...args) {
          if (this.canvas.id === "game" && image.src?.includes("qinling-idle")) {
            window.__idleDraws.push({
              args,
              transform: Array.from(this.getTransform().toFloat64Array()),
            });
            if (window.__idleDraws.length > 1000) window.__idleDraws.shift();
          }
          return draw.call(this, image, ...args);
        };
      });
      await page.clock.install();
      await page.goto(`${base}/scripts/twilight-smoke.html`);
      await page.locator("#start").waitFor();
      await page.clock.pauseAt(await page.evaluate(() => Date.now() + 100));
      await page.evaluate(() => {
        window.__idleDraws = [];
      });
      await page.clock.runFor(4300);
      const draws = await page.evaluate(() => window.__idleDraws);
      const feet = draws.filter(({ args }) => args[7] === 28);
      assert.equal(
        new Set(feet.map(({ args }) => `${args[0]},${args[1]}`)).size,
        6,
        "all six generated poses play"
      );
      assert.equal(
        new Set(feet.map(({ args }) => args[5] + args[7])).size,
        1,
        "foot baseline stays fixed"
      );
      const neutralHeads = draws.filter(({ args }) => args[0] === 148 && args[1] === 25);
      assert(
        new Set(neutralHeads.map(({ args }) => args[5])).size > 1,
        "breathing moves shoulders even between pose changes"
      );
      const torsoHeights = draws
        .filter(({ args }) => args[7] >= 52 && args[7] <= 54)
        .map(({ args }) => args[7]);
      assert.deepEqual(
        [...new Set(torsoHeights)].sort(),
        [52, 53, 54],
        "hoodie expands gently by at most two pixels"
      );
      assert.equal(
        new Set(draws.map(({ transform }) => JSON.stringify(transform))).size,
        1,
        "no whole-sprite bob transform"
      );
      await page.screenshot({ path: path.join(out, `${name}-idle.png`) });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.clock.runFor(30);
      await page.evaluate(() => {
        window.__idleDraws = [];
      });
      await page.clock.runFor(1200);
      const reduced = await page.evaluate(() => window.__idleDraws);
      assert(reduced.length > 0);
      assert(
        reduced.every(({ args }) => args[0] === 148),
        "reduced motion uses neutral frame"
      );
      assert.equal(
        new Set(reduced.filter(({ args }) => args[1] === 25).map(({ args }) => args[5])).size,
        1,
        "reduced motion stops breathing"
      );
      await page.evaluate(() => window.__rhythmController.destroy());
      const before = await page.evaluate(() => window.__idleDraws.length);
      await page.clock.runFor(1000);
      assert.equal(
        await page.evaluate(() => window.__idleDraws.length),
        before,
        "unmount stops animation"
      );
      assert.deepEqual(errors, []);
      await context.close();
    }
    if (process.env.TEST_RECORD) {
      const context = await browser.newContext({
        viewport: { width: 1500, height: 800 },
        recordVideo: { dir: out, size: { width: 1500, height: 800 } },
      });
      const page = await context.newPage();
      await page.goto(`${base}/scripts/twilight-smoke.html`);
      await page.locator("#start").waitFor();
      await page.addStyleTag({
        content: '[id*="vue-devtools"], [class*="vue-devtools"] { display:none!important; }',
      });
      await page.locator("#song-next").click();
      await page.waitForTimeout(9200);
      const video = page.video();
      await context.close();
      await video.saveAs(path.join(out, "idle-preview.webm"));
    }
    console.log(
      "PASS: six-frame idle, grounded feet, no bobbing, reduced motion and cleanup on desktop/iPhone layout"
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
