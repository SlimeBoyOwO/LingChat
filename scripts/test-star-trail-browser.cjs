const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/star-trail");
const base = process.env.TEST_ORIGIN || "http://127.0.0.1:1438";
fs.mkdirSync(out, { recursive: true });
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  const page = await browser.newPage({ viewport: { width: 1500, height: 800 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => {
    window.__testAudio = [];
    const Original = window.AudioContext;
    window.AudioContext = class extends Original {
      constructor(options) {
        super(options);
        window.__testAudio.push(this);
      }
    };
    window.__testSignals = [];
    const add = EventTarget.prototype.addEventListener;
    EventTarget.prototype.addEventListener = function (type, listener, options) {
      if ((this === window || this === document) && options?.signal)
        window.__testSignals.push(options.signal);
      return add.call(this, type, listener, options);
    };
  });
  const hideDevtools = () =>
    page.addStyleTag({
      content: '[id*="vue-devtools"], [class*="vue-devtools"] { display: none !important; }',
    });
  await page.goto(`${base}/scripts/mini-games-smoke.html`);
  await page.getByRole("button", { name: "开始游戏", exact: true }).click();
  await page.getByRole("button", { name: "小游戏", exact: true }).click();
  await page.getByRole("button", { name: "星灯远征", exact: true }).waitFor();
  await page.getByRole("button", { name: "暮色节拍", exact: true }).waitFor();
  await page
    .getByRole("button", { name: "自由对话模式", exact: true })
    .waitFor({ state: "hidden" });
  await hideDevtools();
  await page.screenshot({ path: path.join(out, "mini-games-menu.png") });
  await page.getByRole("button", { name: "星灯远征", exact: true }).click();
  await page.locator("#trail-start").waitFor();
  await page.screenshot({ path: path.join(out, "star-trail-title.png") });
  await page.locator("#trail-start").click();
  await page.waitForFunction(() => window.__testAudio.some((audio) => audio.state === "running"));
  await page.keyboard.down("KeyD");
  await page.keyboard.down("Space");
  await page.waitForTimeout(800);
  await page.keyboard.up("KeyD");
  await page.keyboard.up("Space");
  await page.screenshot({ path: path.join(out, "star-trail-playing.png") });
  await page.keyboard.press("Escape");
  await page.locator("#trail-overlay-exit").click();
  await page.getByRole("button", { name: "星灯远征", exact: true }).waitFor();
  await page.waitForFunction(() => window.__testAudio.every((audio) => audio.state === "closed"));

  await page.goto(`${base}/scripts/star-trail-smoke.html`);
  await hideDevtools();
  await page.locator("#trail-start").waitFor();
  assert.equal(
    await page.evaluate(() => window.__testAudio.length),
    0,
    "no audio before a player gesture"
  );
  for (const size of [
    { width: 1500, height: 800 },
    { width: 1920, height: 1080 },
    { width: 1000, height: 800 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(size);
    const box = await page.locator(".star-trail").boundingBox();
    assert.equal(Math.round(box.width), size.width);
    assert.equal(Math.round(box.height), size.height);
    assert(
      await page
        .locator(".star-trail")
        .evaluate((el) => el.scrollWidth === el.clientWidth && el.scrollHeight === el.clientHeight)
    );
    await page.waitForTimeout(40);
    assert(
      await page
        .locator("#trail-canvas")
        .evaluate(
          (canvas) =>
            Math.abs(canvas.width / canvas.height - canvas.clientWidth / canvas.clientHeight) < 0.01
        )
    );
  }
  await page.screenshot({ path: path.join(out, "star-trail-phone.png") });
  await page.setViewportSize({ width: 1500, height: 800 });
  await page.locator("#trail-help").click();
  await page.locator("#trail-help-close").waitFor();
  await page.keyboard.press("Escape");
  await page.locator("#trail-help-screen").waitFor({ state: "hidden" });
  await page.locator("#trail-start").click();
  await page.waitForFunction(() => window.__trailController.snapshot().audioState === "running");
  const snapshot = () => page.evaluate(() => window.__trailController.snapshot());
  const before = await snapshot();
  await page.keyboard.down("KeyD");
  await page.waitForTimeout(250);
  await page.keyboard.up("KeyD");
  assert((await snapshot()).player.x > before.player.x + 20);
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(260);
  await page.keyboard.up("KeyW");
  assert((await snapshot()).player.y < 220);
  await page.keyboard.down("Space");
  await page.waitForTimeout(250);
  await page.keyboard.up("Space");
  assert((await snapshot()).player.shot > 0);
  await page.keyboard.press("Escape");
  await page.locator("#trail-primary").waitFor();
  await page.waitForFunction(() => window.__trailController.snapshot().audioState === "suspended");
  const paused = await snapshot();
  await page.waitForTimeout(250);
  assert.equal((await snapshot()).time, paused.time);
  await page.locator("#trail-volume").fill("0");
  await page.locator("#trail-volume").dispatchEvent("input");
  assert.equal(await page.locator("#trail-volume-value").textContent(), "0%");
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => window.__trailController.snapshot().mode === "playing");
  await page.keyboard.down("KeyD");
  await page.keyboard.down("Space");
  await page.waitForFunction(() => window.__trailController.snapshot().mode === "dead", null, {
    timeout: 10000,
  });
  await page.keyboard.up("KeyD");
  await page.keyboard.up("Space");
  await page.locator("#trail-primary").click();
  assert.equal((await snapshot()).player.hp, 5);
  assert((await snapshot()).player.x < 80);
  await page.setViewportSize({ width: 800, height: 450 });
  assert(
    await page.locator("#trail-touch").isHidden(),
    "narrow desktop windows do not show phone controls"
  );
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await page.waitForFunction(() => window.__trailController.snapshot().mode === "paused");
  await page.locator("#trail-title-return").click();
  await page.locator("#trail-start").waitFor();
  assert(await page.locator("#trail-announcement").isHidden());
  await page.locator("#trail-start").click();
  assert.equal((await snapshot()).level, 0);
  await page.evaluate(() => window.__trailAbort());
  await page.waitForFunction(() => window.__testAudio.every((audio) => audio.state === "closed"));
  assert(await page.evaluate(() => window.__testSignals.every((signal) => signal.aborted)));
  const destroyed = await snapshot();
  await page.waitForTimeout(100);
  assert.equal((await snapshot()).time, destroyed.time);
  await page.goto(`${base}/scripts/star-trail-smoke.html`);
  await page.locator("#trail-start").click();
  await page.evaluate(() => window.__trailAbort());
  await page.waitForFunction(() => window.__testAudio.every((audio) => audio.state === "closed"));
  assert.deepEqual(errors, []);
  const report = {
    errors,
    checks: [
      "two main-menu games",
      "native route",
      "procedural canvas",
      "fullscreen aspect ratios",
      "help",
      "keyboard movement/jump/fire",
      "pause and audio suspension",
      "volume",
      "death and retry",
      "desktop hides phone controls",
      "blur pause",
      "route audio cleanup",
      "abort and initialization cleanup",
    ],
  };
  fs.writeFileSync(path.join(out, "browser-result.json"), JSON.stringify(report, null, 2) + "\n");
  console.log("PASS:", report.checks.join(", "));
  await browser.close();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
