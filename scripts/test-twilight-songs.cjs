const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium, devices } = require("playwright");
const base = process.env.TEST_ORIGIN || "http://127.0.0.1:1438";
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/twilight-songs");
fs.mkdirSync(out, { recursive: true });
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  try {
    const context = await browser.newContext({
      viewport: { width: 1500, height: 800 },
      userAgent:
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/146.0.0.0 Safari/537.36",
      hasTouch: true,
    });
    const page = await context.newPage(),
      errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => {
      window.__songShift = 0;
      window.__audio = [];
      window.__sources = [];
      window.__workers = [];
      const OriginalAudio = window.AudioContext,
        OriginalWorker = window.Worker;
      window.AudioContext = class extends OriginalAudio {
        constructor(options) {
          super(options);
          window.__audio.push(this);
        }
        getOutputTimestamp() {
          return {
            contextTime: this.currentTime + window.__songShift,
            performanceTime: performance.now(),
          };
        }
        createBufferSource() {
          const source = super.createBufferSource();
          window.__sources.push(source);
          return source;
        }
      };
      window.Worker = class extends OriginalWorker {
        constructor(...args) {
          super(...args);
          window.__workers.push(this);
          this.ended = false;
        }
        terminate() {
          this.ended = true;
          super.terminate();
        }
      };
    });
    await page.goto(`${base}/scripts/twilight-smoke.html`);
    await page.locator("#start").waitFor();
    await page.addStyleTag({
      content: '[id*="vue-devtools"], [class*="vue-devtools"] { display:none!important; }',
    });
    const snap = () => page.evaluate(() => window.__rhythmController.snapshot());
    assert.equal((await snap()).songId, "lantern-echo");
    await page.locator("#song-next").click();
    assert.equal((await snap()).noteCount, 608);
    assert.match(await page.locator("#song-details").textContent(), /168 BPM.*608/);
    await page.screenshot({ path: path.join(out, "neon-title-desktop.png") });
    await page.locator("#start").click();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    assert(
      await page.locator("#touch-keys").isHidden(),
      "touch-capable Windows still hides phone buttons"
    );
    assert(await page.evaluate(() => window.__sources.at(-1).buffer.duration > 83));
    await page.keyboard.down("KeyD");
    assert.deepEqual((await snap()).held, [0]);
    await page.keyboard.up("KeyD");
    await page.locator("#pause").click();
    await page.locator("#leave").click();
    await page.locator("#song-prev").click();
    await page.locator("#demo").click();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    assert.equal((await snap()).result.totalNotes, 138);
    assert(
      await page.evaluate(() => window.__sources.at(-1).buffer.duration < 73),
      "song changes replace audio buffer"
    );
    await page.locator("#pause").click();
    await page.locator("#leave").click();
    await page.locator("#song-next").click();
    await page.locator("#demo").click();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    await page.evaluate(() => {
      window.__songShift = 27;
    });
    await page.waitForFunction(() => window.__rhythmController.snapshot().section === "DROP 01");
    await page.waitForFunction(() => window.__rhythmController.snapshot().result.perfect > 128);
    assert.equal((await snap()).result.miss, 0, "chronological autoplay survives a delayed frame");
    assert.equal((await snap()).result.perfect, (await snap()).result.maxCombo);
    assert((await snap()).effectCount <= 32);
    await page.screenshot({ path: path.join(out, "neon-drop-desktop.png") });
    await page.evaluate(() => {
      window.__songShift = 47.2;
    });
    await page.waitForFunction(() => window.__rhythmController.snapshot().section === "AFTERGLOW");
    await page.waitForFunction(() => window.__rhythmController.snapshot().result.perfect >= 352);
    await page.locator("#pause").click();
    await page.locator("#pause-settings").click();
    await page.locator("#beat-effects").uncheck();
    assert.equal(await page.locator(".cadence-root").getAttribute("data-effects"), "false");
    assert.equal((await snap()).effectCount, 0);
    await page.locator("#beat-effects").check();
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.waitForFunction(
      () =>
        document.querySelector("#rhythm-host").shadowRoot.querySelector(".cadence-root").dataset
          .effects === "false"
    );
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.locator("#settings-close").click();
    // The resumed source seeks to the saved time; remove the artificial clock shift first.
    await page.evaluate(() => {
      window.__songShift = 0;
    });
    await page.locator("#resume").click();
    await page.waitForFunction(() => window.__rhythmController.snapshot().state === "playing");
    await page.evaluate(() => {
      window.__songShift = 84;
    });
    await page.locator("#result-screen").waitFor({ state: "visible" });
    const result = (await snap()).result;
    assert.equal(result.totalNotes, 608);
    assert.equal(result.maxCombo, 608);
    assert.equal(result.miss, 0);
    assert.equal(result.songId, "neon-overdrive");
    assert.match(await page.locator("#result-song").textContent(), /608/);
    await page.screenshot({ path: path.join(out, "neon-result.png") });
    await page.evaluate(() => window.__rhythmAbort());
    await page.waitForFunction(() => window.__audio.every((audio) => audio.state === "closed"));
    assert(await page.evaluate(() => window.__workers.every((worker) => worker.ended)));
    // Abort while worker synthesis is pending; no worker or AudioContext survives the route.
    await page.reload();
    await page.locator("#start").waitFor();
    await page.locator("#start").click();
    await page.evaluate(() => window.__rhythmAbort());
    await page.waitForFunction(
      () =>
        window.__audio.every((audio) => audio.state === "closed") &&
        window.__workers.every((worker) => worker.ended)
    );
    assert.deepEqual(errors, []);

    for (const device of ["Pixel 7", "iPhone 13"]) {
      const phone = await browser.newContext({ ...devices[device] });
      const mobile = await phone.newPage();
      mobile.on("pageerror", (error) => errors.push(error.message));
      await mobile.goto(`${base}/scripts/twilight-smoke.html`);
      await mobile.locator("#start").waitFor();
      await mobile.locator("#song-next").tap();
      for (const size of [
        { width: 320, height: 568 },
        { width: 844, height: 390 },
      ]) {
        await mobile.setViewportSize(size);
        await mobile.evaluate(() => {
          document.documentElement.style.setProperty("--safe-area-inset-top", "20px");
          document.documentElement.style.setProperty("--safe-area-inset-bottom", "34px");
        });
        await mobile.waitForTimeout(100);
        for (const id of ["song-next", "song-prev", "start", "back-to-games"]) {
          const box = await mobile.locator(`#${id}`).boundingBox();
          assert(
            box &&
              box.y >= 20 &&
              box.y + box.height <= size.height - 34 &&
              box.x >= 0 &&
              box.x + box.width <= size.width,
            `${device} ${id} fits safe viewport`
          );
        }
        await mobile.screenshot({
          path: path.join(out, `${device.replaceAll(" ", "-")}-${size.width}.png`),
        });
      }
      await mobile.locator("#start").tap();
      await mobile.locator("#touch-keys").waitFor({ state: "visible" });
      await mobile.locator('[data-lane="0"]').tap();
      assert.equal(await mobile.locator(".cadence-root").getAttribute("data-touch"), "true");
      await mobile.evaluate(() => window.__rhythmAbort());
      await phone.close();
    }
    const mac = await browser.newContext({
      userAgent: devices["Desktop Safari"].userAgent,
      viewport: { width: 390, height: 844 },
    });
    const macPage = await mac.newPage();
    for (const [route, start, control] of [
      ["star-trail", "#trail-start", "#trail-touch"],
      ["twilight", "#start", "#touch-keys"],
    ]) {
      await macPage.goto(`${base}/scripts/${route}-smoke.html`);
      await macPage.locator(start).click();
      await macPage.waitForTimeout(800);
      assert(
        await macPage.locator(control).isHidden(),
        `narrow desktop Mac has no mobile buttons: ${route} ${JSON.stringify(await macPage.locator(control).evaluate((el) => ({ display: getComputedStyle(el).display, dataset: el.parentElement.dataset, ua: navigator.userAgent, touch: navigator.maxTouchPoints })))}`
      );
    }
    await mac.close();
    assert.deepEqual(errors, []);
    fs.writeFileSync(
      path.join(out, "browser-result.json"),
      JSON.stringify(
        {
          errors,
          result,
          checks: [
            "two songs",
            "worker rendering and cancellation",
            "correct audio buffers",
            "desktop keyboard and hidden phone buttons",
            "608-note demo",
            "sections and effects",
            "reduced motion",
            "mobile song picker and input",
            "result metadata",
          ],
        },
        null,
        2
      )
    );
    console.log(
      "PASS: song selection, synthesis, full dense demo, effects, cancellation and desktop/mobile controls"
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
