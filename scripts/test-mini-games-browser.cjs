const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/mini-games");
fs.mkdirSync(out, { recursive: true });
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
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
    window.__testListeners = [];
    const add = EventTarget.prototype.addEventListener;
    EventTarget.prototype.addEventListener = function (type, listener, options) {
      if ((this === window || this === document) && options?.signal)
        window.__testListeners.push({ type, signal: options.signal });
      return add.call(this, type, listener, options);
    };
  });
  await page.goto(process.env.TEST_URL || "http://127.0.0.1:1437/scripts/mini-games-smoke.html");
  await page.getByRole("button", { name: "开始游戏", exact: true }).click();
  await page.getByRole("button", { name: "小游戏", exact: true }).click();
  await page.getByRole("button", { name: "暮色节拍", exact: true }).waitFor();
  assert.equal(await page.locator(".game-card").count(), 0);
  await page.getByAltText("LingChatLogo").waitFor();
  await page
    .getByRole("button", { name: "自由对话模式", exact: true })
    .waitFor({ state: "hidden" });
  await page.screenshot({ path: path.join(out, "mini-games-menu.png"), fullPage: true });
  await page.getByRole("button", { name: "暮色节拍", exact: true }).click();
  await page.locator("#start").waitFor({ state: "visible" });
  assert.equal(
    await page.locator("iframe").count(),
    0,
    "game should run inside the app's main document"
  );
  for (const viewport of [
    { width: 1500, height: 800 },
    { width: 1920, height: 1080 },
    { width: 1000, height: 800 },
  ]) {
    await page.setViewportSize(viewport);
    const area = await page.locator(".cadence-root").boundingBox();
    assert(
      Math.abs(area.width - viewport.width) < 2 && Math.abs(area.height - viewport.height) < 2
    );
    assert.equal(await page.locator(".masthead, .below-stage, .game-card, footer").count(), 0);
    assert(
      await page
        .locator(".twilight-rhythm-page")
        .evaluate((el) => el.scrollHeight === el.clientHeight && el.scrollWidth === el.clientWidth)
    );
    const scene = await page.locator("#scene-background").boundingBox();
    assert(
      Math.abs(scene.width - viewport.width) < 2 && Math.abs(scene.height - viewport.height) < 2
    );
  }
  await page.setViewportSize({ width: 1500, height: 800 });
  await page.locator("#settings-toggle").click();
  await page.locator("#settings-close").waitFor({ state: "visible" });
  await page.keyboard.press("Escape");
  await page.locator("#settings").waitFor({ state: "hidden" });
  await page.mouse.click(750, 40);
  await page.screenshot({ path: path.join(out, "native-rhythm.png"), fullPage: true });
  await page.locator("#start").click();
  await page.locator("#live-tools").waitFor({ state: "visible" });
  await page.waitForTimeout(2600);
  await page.keyboard.down("d");
  assert(await page.locator('[data-lane="0"]').evaluate((el) => el.classList.contains("active")));
  await page.keyboard.up("d");
  await page.screenshot({ path: path.join(out, "playing-fullscreen.png"), fullPage: true });
  await page.keyboard.press("Escape");
  await page.locator("#pause-screen").waitFor({ state: "visible" });
  await page.locator("#resume").click();
  await page.waitForTimeout(1850);
  assert(!(await page.locator("#footer-status").textContent()).includes("已暂停"));
  await page.keyboard.press("Escape");
  await page.locator("#exit-paused").click();
  await page.getByRole("button", { name: "暮色节拍", exact: true }).waitFor();
  await page.waitForFunction(() =>
    window.__testAudio.every((context) => context.state === "closed")
  );
  assert(
    await page.evaluate(() => window.__testListeners.every((listener) => listener.signal.aborted))
  );
  // A second mount must get a new clock, without reviving listeners from the first one.
  await page.getByRole("button", { name: "暮色节拍", exact: true }).click();
  await page.locator("#start").waitFor({ state: "visible" });
  await page.locator("#demo").click();
  await page.locator("#live-tools").waitFor({ state: "visible" });
  await page.evaluate(() => window.__miniGameRouter.push("/mini-games"));
  await page.waitForFunction(
    () =>
      window.__testAudio.length === 2 &&
      window.__testAudio.every((context) => context.state === "closed")
  );
  assert(
    await page.evaluate(() => window.__testListeners.every((listener) => listener.signal.aborted))
  );
  // Cancel during asset initialization, then verify the game can mount again.
  await page.evaluate(async () => {
    await window.__miniGameRouter.push("/mini-games/twilight");
    await window.__miniGameRouter.push("/mini-games");
  });
  await page.getByRole("button", { name: "暮色节拍", exact: true }).click();
  await page.locator("#start").waitFor({ state: "visible" });
  await page.setViewportSize({ width: 390, height: 844 });
  const box = await page.locator("#start").boundingBox();
  assert(box.y + box.height <= 844);
  const extent = await page.locator(".cadence-root").boundingBox();
  assert(Math.abs(extent.width - 390) < 2 && Math.abs(extent.height - 844) < 2);
  await page.screenshot({ path: path.join(out, "native-phone.png"), fullPage: true });
  await page.locator("#back-to-games").click();
  await page.getByRole("button", { name: "返回", exact: true }).click();
  await page.getByRole("button", { name: "小游戏", exact: true }).waitFor();
  assert.deepEqual(errors, []);
  const report = {
    errors,
    checks: [
      "existing menu entry",
      "shared main-menu selection",
      "fullscreen scene",
      "no page scrolling",
      "in-scene settings",
      "native canvas",
      "bundled assets",
      "keyboard",
      "pause/resume",
      "return closes audio",
      "route unmount cleanup",
      "repeat mount",
      "initialization cancellation",
      "narrow layout",
      "return to main menu",
    ],
  };
  fs.writeFileSync(path.join(out, "result.json"), JSON.stringify(report, null, 2) + "\n");
  console.log("PASS:", report.checks.join(", "));
  await browser.close();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
