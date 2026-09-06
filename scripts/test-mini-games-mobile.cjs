const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium, webkit } = require("playwright");
const base = process.env.TEST_ORIGIN || "http://127.0.0.1:1438";
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/mobile");
fs.mkdirSync(out, { recursive: true });
const engines = process.env.TEST_ENGINE ? [process.env.TEST_ENGINE] : ["chromium", "webkit"];
const sizes = [
  { width: 390, height: 844 },
  { width: 844, height: 390 },
  { width: 320, height: 568 },
  { width: 667, height: 375 },
  { width: 768, height: 1024 },
  { width: 1024, height: 768 },
];
async function inset(page, size) {
  await page.setViewportSize(size);
  const safe =
    size.width < size.height
      ? { top: 47, right: 0, bottom: 34, left: 0 }
      : { top: 0, right: 44, bottom: 21, left: 44 };
  await page.evaluate((safe) => {
    for (const [edge, value] of Object.entries(safe))
      document.documentElement.style.setProperty(`--safe-area-inset-${edge}`, `${value}px`);
  }, safe);
  await page.waitForTimeout(100);
  return safe;
}
async function fits(page, selector, safe, minimum = 0) {
  await page.locator(selector).waitFor({ state: "visible" });
  const box = await page.locator(selector).boundingBox(),
    size = page.viewportSize();
  assert(box, `${selector} is visible`);
  assert(
    box.width >= minimum && box.height >= minimum,
    `${selector} touch target ${JSON.stringify(box)}`
  );
  assert(
    box.x >= safe.left - 1 &&
      box.y >= safe.top - 1 &&
      box.x + box.width <= size.width - safe.right + 1 &&
      box.y + box.height <= size.height - safe.bottom + 1,
    `${selector} safe area ${JSON.stringify({ box, safe, size })}`
  );
}
async function center(page, selector, id) {
  await page.locator(selector).waitFor({ state: "visible" });
  const box = await page.locator(selector).boundingBox();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2, id };
}
async function checkWithoutWebAudio(page, engine) {
  // The Windows WebKit port may omit Web Audio entirely. Keep that limitation
  // explicit: exercise real UI/input where possible, never substitute a fake clock.
  for (const size of sizes) {
    const safe = await inset(page, size);
    await fits(page, "#start", safe, 48);
    await fits(page, "#back-to-games", safe, 48);
    await page.locator("#settings-toggle").tap();
    await fits(page, ".settings-panel", safe);
    await page.locator("#settings-close").tap();
    await page.screenshot({
      path: path.join(out, `${engine}-rhythm-title-${size.width}x${size.height}.png`),
    });
  }
  await page.goto(`${base}/scripts/star-trail-smoke.html`);
  await page.locator("#trail-start").tap();
  for (const size of sizes) {
    const safe = await inset(page, size);
    if (await page.evaluate(() => window.__trailController.snapshot().mode === "paused"))
      await page.locator("#trail-primary").tap();
    for (const action of ["left", "right", "jump", "fire"])
      await fits(page, `[data-action="${action}"]`, safe, 48);
    await page.locator('[data-action="jump"]').tap();
    await page.waitForTimeout(70);
    assert(
      await page.evaluate(() => window.__trailController.snapshot().player.y < 270),
      "WebKit touch jump without audio"
    );
    await page.locator("#trail-pause").tap();
    await page.locator("#trail-shop-open").tap();
    await fits(page, ".shop-panel", safe);
    await page.locator("#trail-shop-close").tap();
    await page.locator("#trail-gear-open").tap();
    await fits(page, ".gear-panel", safe);
    assert.equal(
      await page.locator(".gear-panel").evaluate((el) => el.scrollTop),
      0,
      "equipment opens at its heading"
    );
    await page.screenshot({
      path: path.join(out, `${engine}-armor-${size.width}x${size.height}.png`),
    });
    await page.locator("#trail-gear-close").tap();
    await page.locator("#trail-title-return").tap();
    await page.locator("#trail-help").tap();
    await fits(page, "#trail-help-screen .overlay-menu", safe);
    await page.locator("#trail-help-close").tap();
    await page.locator("#trail-start").tap();
    await page.waitForTimeout(650);
  }
  await page.evaluate(() => window.__trailAbort());
  await page.goto(`${base}/scripts/mini-games-smoke.html`);
  await page.getByRole("button", { name: "开始游戏", exact: true }).tap();
  await page.getByRole("button", { name: "小游戏", exact: true }).tap();
  for (const [title, back] of [
    ["暮色节拍", "#back-to-games"],
    ["星灯远征", "#trail-exit"],
  ]) {
    await page.getByRole("button", { name: title, exact: true }).tap();
    await page.locator(back).tap();
  }
  return {
    engine,
    sizes,
    checks: [
      "touch menus",
      "safe areas",
      "scrollable panels",
      "platformer touch gameplay",
      "native routes",
    ],
    skipped: ["rhythm playback and holds", "audio interruption and resume"],
    reason: "This WebKit build exposes neither AudioContext nor webkitAudioContext",
  };
}
async function run(engine) {
  const browser = await (engine === "webkit" ? webkit : chromium).launch({
    headless: true,
    ...(engine === "chromium" ? { executablePath: process.env.CHROME_PATH } : {}),
  });
  try {
    const context = await browser.newContext({
      viewport: sizes[0],
      isMobile: true,
      hasTouch: true,
      deviceScaleFactor: 2,
    });
    const page = await context.newPage();
    page.setDefaultTimeout(12000);
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => {
      window.__audio = [];
      const Original = window.AudioContext || window.webkitAudioContext;
      if (!Original) return;
      window.AudioContext = class extends Original {
        constructor(options) {
          super(options);
          window.__audio.push(this);
        }
      };
    });
    const snap = (kind) =>
      page.evaluate(
        (kind) => window[kind === "rhythm" ? "__rhythmController" : "__trailController"].snapshot(),
        kind
      );
    const shot = (name) => page.screenshot({ path: path.join(out, `${engine}-${name}.png`) });
    const hideDevtools = () =>
      page.addStyleTag({
        content: '[id*="vue-devtools"], [class*="vue-devtools"] { display:none!important; }',
      });
    await page.goto(`${base}/scripts/twilight-smoke.html`);
    await page.locator("#start").waitFor();
    await hideDevtools();
    if (!(await page.evaluate(() => !!(window.AudioContext || window.webkitAudioContext)))) {
      const report = await checkWithoutWebAudio(page, engine);
      assert.deepEqual(errors, []);
      fs.writeFileSync(
        path.join(out, `${engine}-result.json`),
        JSON.stringify({ ...report, errors }, null, 2)
      );
      console.log(
        `PASS ${engine}: touch UI and platformer; SKIP Web Audio: unavailable in this browser build`
      );
      return;
    }
    let safe = await inset(page, sizes[0]);
    await fits(page, "#start", safe, 48);
    await page.locator("#start").tap();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    assert.equal((await snap("rhythm")).audioState, "running");
    for (const size of sizes) {
      safe = await inset(page, size);
      if ((await snap("rhythm")).state === "paused") {
        assert.deepEqual((await snap("rhythm")).held, []);
        await page.locator("#resume").tap();
        await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
      }
      for (let lane = 0; lane < 4; lane++) await fits(page, `[data-lane="${lane}"]`, safe, 48);
      await fits(page, "#pause", safe, 48);
      const canvas = await page
        .locator("#game")
        .evaluate((el) => ({ w: el.width, h: el.height, cw: el.clientWidth, ch: el.clientHeight }));
      assert(Math.abs(canvas.w / canvas.h - canvas.cw / canvas.ch) < 0.01, "square rhythm pixels");
      await shot(`rhythm-${size.width}x${size.height}`);
    }
    safe = await inset(page, sizes[0]);
    if ((await snap("rhythm")).state === "paused") {
      await page.locator("#resume").tap();
      await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    }
    if (engine === "chromium") {
      const cdp = await context.newCDPSession(page);
      const points = await Promise.all(
        [0, 1, 2, 3].map((lane) => center(page, `[data-lane="${lane}"]`, lane + 1))
      );
      await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: points });
      assert.deepEqual((await snap("rhythm")).held.sort(), [0, 1, 2, 3], "real four finger chord");
      await page.waitForTimeout(300);
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchEnd",
        touchPoints: points.slice(0, 1),
      });
      assert.deepEqual(
        (await snap("rhythm")).held.sort(),
        [1, 2, 3],
        "independent long press release"
      );
      await cdp.send("Input.dispatchTouchEvent", { type: "touchCancel", touchPoints: [] });
      assert.deepEqual((await snap("rhythm")).held, [], "system gesture cancels all lanes");
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchStart",
        touchPoints: points.slice(0, 1),
      });
      await inset(page, sizes[1]);
      assert.equal((await snap("rhythm")).state, "paused", "rotation pauses held notes");
      assert.deepEqual((await snap("rhythm")).held, []);
      await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      await cdp.detach();
    } else {
      const point = await center(page, '[data-lane="0"]');
      await page.mouse.move(point.x, point.y);
      await page.mouse.down();
      assert.deepEqual((await snap("rhythm")).held, [0], "WebKit pointer hold");
      await page.mouse.up();
      assert.deepEqual((await snap("rhythm")).held, []);
      await page.locator('[data-lane="1"]').tap();
      await inset(page, sizes[1]);
      assert.equal((await snap("rhythm")).state, "paused");
    }
    await page.locator("#pause-settings").tap();
    await fits(page, ".settings-panel", await inset(page, sizes[1]));
    await page.locator("#settings-close").tap();
    await page.locator("#resume").tap();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    await page.evaluate(() => window.__audio[0].suspend());
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "paused");
    await page.locator("#resume").tap();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    assert.equal((await snap("rhythm")).audioState, "running");
    await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "paused");
    await page.locator("#resume").tap();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    await page.evaluate(() => window.__rhythmAbort());
    await page.waitForFunction(() => window.__audio.every((audio) => audio.state === "closed"));

    await page.goto(`${base}/scripts/star-trail-smoke.html`);
    await page.locator("#trail-start").waitFor();
    await hideDevtools();
    safe = await inset(page, sizes[0]);
    await page.locator("#trail-start").tap();
    await page.waitForFunction(() => window.__trailController.snapshot().audioState === "running");
    for (const size of sizes) {
      safe = await inset(page, size);
      if ((await snap("trail")).mode === "paused") await page.locator("#trail-primary").tap();
      for (const action of ["left", "right", "jump", "fire"])
        await fits(page, `[data-action="${action}"]`, safe, 48);
      await fits(page, "#trail-pause", safe, 48);
      await fits(page, "#trail-armor-open", safe, 48);
      assert(
        await page
          .locator("#trail-canvas")
          .evaluate(
            (el) =>
              el.width >= 480 &&
              Math.abs(el.width / el.height - el.clientWidth / el.clientHeight) < 0.01
          ),
        "portrait world retains visibility and square pixels"
      );
      await shot(`trail-${size.width}x${size.height}`);
      await page.locator("#trail-pause").tap();
      await fits(page, "#trail-overlay .overlay-menu", safe);
      await page.locator("#trail-shop-open").tap();
      await fits(page, ".shop-panel", safe);
      await page.locator("#trail-shop-close").tap();
      await page.locator("#trail-gear-open").tap();
      await fits(page, ".gear-panel", safe);
      await page.locator("#trail-gear-close").tap();
      await page.locator("#trail-primary").tap();
    }
    safe = await inset(page, sizes[0]);
    if ((await snap("trail")).mode === "paused") await page.locator("#trail-primary").tap();
    if (engine === "chromium") {
      const cdp = await context.newCDPSession(page);
      const points = await Promise.all(
        ["right", "jump", "fire"].map((action, i) =>
          center(page, `[data-action="${action}"]`, i + 1)
        )
      );
      const before = await snap("trail");
      await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: points });
      await page.waitForTimeout(240);
      const after = await snap("trail");
      assert(
        after.player.x > before.player.x + 20 &&
          after.player.y < before.player.y - 30 &&
          after.player.shot > 0,
        "three fingers move, jump and shoot together"
      );
      const left = await center(page, '[data-action="left"]', 1);
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchMove",
        touchPoints: [left, ...points.slice(1)],
      });
      assert(
        await page
          .locator('[data-action="left"]')
          .evaluate((el) => el.classList.contains("active")),
        "slide direction"
      );
      assert(
        !(await page
          .locator('[data-action="right"]')
          .evaluate((el) => el.classList.contains("active")))
      );
      await cdp.send("Input.dispatchTouchEvent", { type: "touchCancel", touchPoints: [] });
      assert.equal(await page.locator(".touch-controls .active").count(), 0);
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchStart",
        touchPoints: points.slice(0, 1),
      });
      await inset(page, sizes[1]);
      assert.equal((await snap("trail")).mode, "paused");
      assert.equal(await page.locator(".touch-controls .active").count(), 0);
      await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      await cdp.detach();
    } else {
      await page.locator('[data-action="jump"]').tap();
      await page.waitForTimeout(70);
      assert((await snap("trail")).player.y < 270, "WebKit touch jump");
      await inset(page, sizes[1]);
    }
    await page.locator("#trail-primary").tap();
    await page.waitForFunction(() => window.__trailController.snapshot().audioState === "running");
    await page.evaluate(() => window.__audio[0].suspend());
    await page.waitForFunction(() => window.__trailController.snapshot().mode === "paused");
    await page.locator("#trail-primary").tap();
    await page.waitForFunction(() => window.__trailController.snapshot().audioState === "running");
    await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
    await page.waitForFunction(() => window.__trailController.snapshot().mode === "paused");
    await page.evaluate(() => window.__trailAbort());
    await page.waitForFunction(() => window.__audio.every((audio) => audio.state === "closed"));
    await page.goto(`${base}/scripts/mini-games-smoke.html`);
    await page.getByRole("button", { name: "开始游戏", exact: true }).tap();
    await page.getByRole("button", { name: "小游戏", exact: true }).tap();
    for (const [title, back] of [
      ["暮色节拍", "#back-to-games"],
      ["星灯远征", "#trail-exit"],
    ]) {
      await page.getByRole("button", { name: title, exact: true }).tap();
      await page.locator(back).tap();
    }
    assert.deepEqual(errors, []);
    fs.writeFileSync(
      path.join(out, `${engine}-result.json`),
      JSON.stringify(
        {
          engine,
          sizes,
          errors,
          checks: [
            "touch targets",
            "safe insets",
            "portrait and landscape",
            "menu scrolling",
            "pointer input",
            "rotation pause",
            "audio interruption and gesture resume",
            "cleanup",
          ],
        },
        null,
        2
      )
    );
    console.log(`PASS ${engine}: mobile layouts, inputs, audio and cleanup`);
  } finally {
    await browser.close();
  }
}
(async () => {
  for (const engine of engines) await run(engine);
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
