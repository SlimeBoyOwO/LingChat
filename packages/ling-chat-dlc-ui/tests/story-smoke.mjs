import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL, fileURLToPath } from "node:url";
import { resolve } from "node:path";
import assert from "node:assert/strict";
if (!process.env.LINGCHAT_HOST)
  throw Error("Set LINGCHAT_HOST to the matching host checkout");
if (!process.env.LINGCHAT_STORY_ROOT)
  throw Error("Set LINGCHAT_STORY_ROOT to the story's script directory");
const host = resolve(process.env.LINGCHAT_HOST).replaceAll("\\", "/");
const root = resolve(process.env.AUDIT_OUTPUT || ".story-audit");
const sdk = fileURLToPath(new URL("../src", import.meta.url)).replaceAll(
  "\\",
  "/",
);
await mkdir(root, { recursive: true });
const require = createRequire(host + "/package.json");
const { createServer, transformWithEsbuild } = await import(
  pathToFileURL(host + "/node_modules/vite/dist/node/index.js")
);
const vue = (await import(pathToFileURL(require.resolve("@vitejs/plugin-vue"))))
  .default;
const tailwind = (
  await import(pathToFileURL(require.resolve("@tailwindcss/vite")))
).default;
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright");
const errors = [];
const story = resolve(process.env.LINGCHAT_STORY_ROOT).replaceAll("\\", "/");
const roots = {
  storybg: story + "/Assets/Backgrounds",
  Musics: story + "/Assets/Musics",
  pics: story + "/Assets/Pics",
  Sounds: story + "/Assets/Sounds",
  main: host + "/data/game_data/characters/DeepSeek/avatar",
  ql: story + "/characters/钦灵/avatar",
  bg: host + "/data/game_data/backgrounds",
};
const server = await createServer({
  configFile: false,
  root: host,
  plugins: [
    vue(),
    tailwind(),
    {
      name: "story-audit",
      configureServer(s) {
        s.middlewares.use(async (req, res, next) => {
          if (req.url === "/__audit") {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<html><body style="margin:0;width:100vw;height:100vh;background:#102330"><div id="app"></div><script type="module" src="/__audit-entry.ts"></script></body></html>',
            );
            return;
          }
          if (req.url?.startsWith("/__asset/")) {
            try {
              const [, type, name] =
                decodeURIComponent(req.url).match(
                  /^\/__asset\/(main|ql|bg|pics|Sounds|Musics|storybg)\/([^/]+)$/,
                ) || [];
              if (!type) throw Error("bad asset");
              res.setHeader(
                "Content-Type",
                name.endsWith(".ogg")
                  ? "audio/ogg"
                  : name.endsWith(".png")
                    ? "image/png"
                    : "image/webp",
              );
              res.end(await readFile(roots[type] + "/" + name));
            } catch {
              res.statusCode = 404;
              res.end();
            }
            return;
          }
          next();
        });
      },
      resolveId(id) {
        if (id === "/__audit-entry.ts") return "\0story-audit-entry.ts";
      },
      async load(id) {
        if (id === "\0story-audit-entry.ts")
          return (
            await transformWithEsbuild(
              await readFile(
                new URL("./frontend-entry.ts", import.meta.url),
                "utf8",
              ),
              "story-audit-entry.ts",
              { loader: "ts", target: "esnext" },
            )
          ).code;
      },
    },
  ],
  resolve: {
    alias: {
      "@": host + "/src",
      "@lingchat/host": host + "/src",
      "@lingchat/dlc-ui": sdk,
    },
    dedupe: ["vue", "pinia", "vue-router", "vue-i18n"],
  },
  server: { host: "127.0.0.1", port: 0, hmr: false },
  logLevel: "error",
});
let browser;
try {
  await server.listen();
  browser = await chromium.launch({
    executablePath: process.env.BROWSER_PATH || undefined,
    headless: true,
    args: ["--no-proxy-server", "--autoplay-policy=no-user-gesture-required"],
  });
  const page = await browser.newPage({
    viewport: { width: 1500, height: 800 },
  });
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto(server.resolvedUrls.local[0] + "__audit");
  await page.waitForFunction(() => !!window.__audit, { timeout: 30000 });
  const send = (name, payload) =>
    page.evaluate(({ name, payload }) => window.__audit.emit(name, payload), {
      name,
      payload,
    });
  await send("script:chapter-change", { chapterName: "1-1 文学测试部" });
  await send("script:narration", { text: "下午，你推开文学测试部的门。" });
  await page.locator(".response-display").waitFor({ state: "visible" });
  await page.waitForFunction(() =>
    document
      .querySelector(".response-display")
      ?.textContent?.includes("文学测试部"),
  );
  await page.locator(".response-display").press("Enter");
  await send("script:modify-character", {
    characterId: 4,
    action: "show_character",
    emotion: "高兴",
  });
  await send("ai:reply", {
    roleId: 4,
    message: "欢迎来到第七次测试。",
    emotion: "高兴",
    originalTag: "高兴",
    isFinal: false,
  });
  await page.locator(".response-display").press("Enter");
  await page.waitForFunction(() =>
    document
      .querySelector(".response-display")
      ?.textContent?.includes("欢迎来到"),
  );
  await page.locator(".response-display").press("Enter");
  await page.locator(".response-display").press("Enter");
  await send("script:modify-character", {
    characterId: 10,
    action: "show_character",
    emotion: "正常",
  });
  await send("ai:reply", {
    roleId: 10,
    message: "我是钦灵，你能看见我吗？",
    emotion: "正常",
    originalTag: "正常",
    isFinal: false,
  });
  await page.waitForFunction(() =>
    document
      .querySelector(".response-display")
      ?.textContent?.includes("我是钦灵"),
  );
  await page.screenshot({ path: root + "/characters-dialogue.png" });
  assert.deepEqual(
    await page.evaluate(() => window.__audit.game.presentRoleIds),
    [4, 10],
  );
  await page.locator(".response-display").press("Enter");
  await page.locator(".response-display").press("Enter");
  await send("script:background-effect", { effect: "BloodUI+UiCorrupt" });
  await send("script:force-choice", {
    requestId: "fixture-1",
    choices: [{ text: "先离开" }, { text: "开始第七次测试" }],
    forced: "开始第七次测试",
  });
  await page
    .locator(".force-choice-overlay")
    .waitFor({ state: "visible", timeout: 10000 });
  assert(
    await page
      .getByRole("button", { name: "先离开", exact: true })
      .isDisabled(),
  );
  await page.screenshot({ path: root + "/force-choice.png" });
  await page
    .getByRole("button", { name: "开始第七次测试", exact: true })
    .click();
  await page.waitForFunction(() =>
    window.__audit.calls.some((x) => x.cmd === "script_submit_choice"),
  );
  await send("script:narration", { text: "选择已接收，剧情继续。" });
  await page.waitForFunction(() =>
    document
      .querySelector(".response-display")
      ?.textContent?.includes("剧情继续"),
  );
  await page.locator(".response-display").press("Enter");
  await page.locator(".response-display").press("Enter");
  await send("script:background-effect", { effect: "none" });
  await send("script:voice-shift", { rate: 0.7, pitch: -3 });
  await send("script:window-title", { title: "第七次测试" });
  await send("script:glitch-window", { requestId: "glitch-fixture" });
  await send("script:console-window", { requestId: "console-fixture" });
  await page.waitForFunction(() =>
    window.__audit.calls.some((x) => x.cmd === "spawn_script_console_window"),
  );
  assert.equal(await page.evaluate(() => window.__audit.ui.voiceRate), 0.7);
  assert.equal(
    await page.evaluate(
      () =>
        window.__audit.calls.find((x) => x.cmd === "show_script_glitch_window")
          ?.args.requestId,
    ),
    "glitch-fixture",
  );
  for (const mode of ["normal", "act2", "act2_final"]) {
    const rounds = Array.from({ length: 20 }, (_, r) =>
      Array.from({ length: 10 }, (_, i) => ({
        text: `词语${r}-${i}`,
        warmPoints: 3,
        scriptPoints: 2,
        voidPoints: 1,
        glitch: mode === "act2_final" && r === 0 && i === 0,
      })),
    );
    await send("script:poem-game", {
      requestId: `poem-${mode}`,
      rounds,
      mode,
      backgroundPath: "/__asset/storybg/深夜诗笺-半页.png",
      musicPath: mode === "normal" ? "" : "/__asset/Musics/4.ogg",
      glitchMusicPath: "/__asset/Musics/4g.ogg",
      warmStickerPath: "/__asset/pics/写诗Q版-她.png",
      scriptStickerPath: "/__asset/pics/写诗Q版-钦灵.png",
      voidStickerPath: "/__asset/pics/写诗Q版-空白.png",
    });
    await page.locator(".poem-game-overlay").waitFor({ state: "visible" });
    for (let i = 0; i < 20; i++) {
      await page.locator(".poem-word").first().click();
      if (i < 19)
        await page.waitForFunction(
          (round) =>
            document
              .querySelector(".poem-word")
              ?.textContent?.includes(`词语${round}-`),
          i + 1,
        );
      if (i === 0) await page.screenshot({ path: root + `/poem-${mode}.png` });
    }
    await page
      .locator(".poem-game-overlay")
      .waitFor({ state: "hidden", timeout: 10000 });
    const submission = await page.evaluate(
      (mode) =>
        window.__audit.calls.find(
          (x) =>
            x.cmd === "script_submit_poem" &&
            x.args.requestId === `poem-${mode}`,
        )?.args.result,
      mode,
    );
    assert.deepEqual(submission, {
      winner: mode === "normal" ? "warm" : "script",
      glitch: mode === "act2_final",
      warm: 60,
      script: 40,
      void: 20,
    });
  }
  await send("script:wait", { duration: 0.25 });
  await send("script:jumpscare", {
    imagePath: "/__asset/ql/崩坏1.webp",
    duration: 0.25,
    soundPath: "",
  });
  await send("script:narration", { text: "计时演出后仍可继续。" });
  await page.waitForFunction(() =>
    document
      .querySelector(".response-display")
      ?.textContent?.includes("计时演出后"),
  );
  await send("script:watch-jump", {});
  await send("script:window-title-reset", {});
  assert.equal(await page.evaluate(() => window.__audit.game.poemGame), null);
  assert.equal(
    await page.evaluate(() => window.__audit.game.forceChoice),
    null,
  );

  const result = await page.evaluate(() => ({
    stage: window.__audit.game.presentRoleIds,
    chapter: window.__audit.game.runningScript.currentChapterName,
    avatarRequests: window.__audit.calls.filter(
      (x) => x.cmd === "get_avatar_file",
    ),
    poems: window.__audit.calls.filter((x) => x.cmd === "script_submit_poem"),
    choices: window.__audit.calls.filter(
      (x) => x.cmd === "script_submit_choice",
    ),
  }));
  assert.deepEqual(errors, []);
  await writeFile(
    root + "/frontend-result.json",
    JSON.stringify({ result, errors }, null, 2),
  );
  console.log(JSON.stringify({ result, errors }, null, 2));
} catch (e) {
  await writeFile(
    root + "/frontend-errors.json",
    JSON.stringify(errors, null, 2),
  );
  throw e;
} finally {
  await browser?.close();
  await server.close();
}
