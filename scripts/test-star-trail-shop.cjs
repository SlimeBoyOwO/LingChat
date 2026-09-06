const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/star-trail-shop");
const origin = process.env.TEST_ORIGIN || "http://127.0.0.1:1438";
fs.mkdirSync(out, { recursive: true });
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  try {
    const page = await browser.newPage({ viewport: { width: 1500, height: 800 } });
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`${origin}/scripts/star-trail-smoke.html`);
    await page.locator("#trail-start").waitFor();
    await page.addStyleTag({
      content: '[id*="vue-devtools"], [class*="vue-devtools"] { display: none !important; }',
    });
    // Seed a shopping scenario in the development harness; production has no state override.
    await page.evaluate(async () => {
      const { Adventure } = await import("/src/minigames/star-trail/core.js");
      const original = Adventure.prototype.start;
      Adventure.prototype.start = function () {
        original.call(this);
        this.wallet = 30;
        this.player.hp = 3;
        this.player.armor = 0;
      };
    });
    const state = () => page.evaluate(() => window.__trailController.snapshot());
    await page.locator("#trail-start").click();
    await page.locator("#trail-armor-open").click();
    await page.locator("#trail-gear-screen").waitFor({ state: "visible" });
    assert.equal((await state()).mode, "paused");
    assert.equal(await page.locator("#trail-armor-bar").getAttribute("aria-valuenow"), "0");
    await page.locator("#trail-gear-shop").click();
    await page.locator("#trail-shop-screen").waitFor({ state: "visible" });
    const buy = (id) => page.locator(`[data-buy="${id}"]`).click();
    await buy("heal");
    assert.equal((await state()).player.hp, 5);
    assert.equal((await state()).wallet, 26);
    assert(await page.locator('[data-buy="heal"]').isDisabled());
    await buy("repair");
    assert.equal((await state()).player.armor, 2);
    assert.equal((await state()).wallet, 21);
    await buy("upgrade");
    assert.equal((await state()).armorLevel, 1);
    assert.equal((await state()).player.armor, 4);
    assert.equal((await state()).wallet, 11);
    await page.screenshot({ path: path.join(out, "shop.png") });
    await buy("shield");
    assert.equal((await state()).player.shield, 1);
    assert.equal((await state()).wallet, 5);
    await buy("magnet");
    assert.equal((await state()).player.magnet, 10);
    assert.equal((await state()).wallet, 0);
    assert(await page.locator('[data-buy="rapid"]').isDisabled());
    assert((await page.locator('[data-buy="rapid"]').textContent()).includes("还差 6 星晶"));
    const pausedTime = (await state()).time;
    await page.waitForTimeout(200);
    assert.equal((await state()).time, pausedTime);
    assert.equal((await state()).player.magnet, 10);
    for (const size of [
      { width: 800, height: 450 },
      { width: 390, height: 844 },
      { width: 1500, height: 800 },
    ]) {
      await page.setViewportSize(size);
      const panel = await page.locator(".shop-panel").boundingBox();
      assert(
        panel.y >= 0 && panel.y + panel.height <= size.height + 1,
        `shop clipped at ${size.width}x${size.height}`
      );
      const button = await page.locator("#trail-shop-close").boundingBox();
      assert(button.y + button.height <= size.height + 1);
    }
    await page.keyboard.press("Escape");
    await page.locator("#trail-gear-screen").waitFor({ state: "visible" });
    assert.equal(await page.locator("#trail-gear-name").textContent(), "巡星护甲");
    assert.equal(await page.locator("#trail-gear-bar .full").count(), 4);
    await page.screenshot({ path: path.join(out, "armor.png") });
    for (const size of [
      { width: 800, height: 450 },
      { width: 390, height: 844 },
    ]) {
      await page.setViewportSize(size);
      const panel = await page.locator(".gear-panel").boundingBox();
      assert(
        panel.y >= 0 && panel.y + panel.height <= size.height + 1,
        `armor UI clipped at ${size.width}x${size.height}`
      );
    }
    await page.keyboard.press("Escape");
    assert.equal((await state()).mode, "paused");
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => window.__trailController.snapshot().mode === "playing");
    await page.waitForTimeout(150);
    assert((await state()).player.magnet < 10);
    await page.setViewportSize({ width: 1500, height: 800 });
    await page.screenshot({ path: path.join(out, "armor-hud.png") });
    await page.keyboard.press("Escape");
    await page.locator("#trail-shop-open").click();
    await page.evaluate(() => window.__trailAbort());
    await page.waitForFunction(() => window.__trailController.snapshot().audioState === "closed");
    await page.goto(`${origin}/scripts/star-trail-smoke.html`);
    await page.locator("#trail-start").waitFor();
    await page.evaluate(async () => {
      const { Adventure } = await import("/src/minigames/star-trail/core.js");
      const original = Adventure.prototype.start;
      Adventure.prototype.start = function () {
        original.call(this);
        this.player.x = this.level.shop.x + 20;
        this.player.y = this.level.shop.y + this.level.shop.h - this.player.h;
      };
    });
    await page.locator("#trail-start").click();
    await page.locator("#trail-world-shop").waitFor({ state: "visible" });
    await page.keyboard.press("KeyE");
    await page.locator("#trail-shop-screen").waitFor({ state: "visible" });
    await page.keyboard.press("Escape");
    await page.locator("#trail-primary").click();
    await page.locator("#trail-world-shop").click();
    await page.locator("#trail-shop-screen").waitFor({ state: "visible" });
    await page.evaluate(() => window.__trailAbort());
    assert.deepEqual(errors, []);
    const report = {
      errors,
      checks: [
        "armor HUD opens equipment",
        "health and armor repair",
        "armor upgrade",
        "shield and magnet purchase",
        "currency and disabled reasons",
        "paused timers",
        "shop and equipment layouts",
        "nested Escape navigation",
        "HUD refresh",
        "cleanup while shopping",
        "world shop keyboard and pointer entry",
      ],
    };
    fs.writeFileSync(path.join(out, "shop-result.json"), JSON.stringify(report, null, 2) + "\n");
    console.log("PASS:", report.checks.join(", "));
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
