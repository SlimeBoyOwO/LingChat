const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const ts = require("typescript");

// Execute the real component logic with fake IPC; this test never moves a cursor.
const source = fs.readFileSync(
  path.join(__dirname, "../src/components/game/standard/extra/ForceChoice.vue"),
  "utf8"
);
const setup = source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1];
const code = ts.transpileModule(
  setup +
    `\nexports.test = {
  tick, onChoiceClick,
  attach(root, panel) { overlayRef.value = root; choicesRef.value = panel },
  generation: () => runGeneration
}`,
  { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }
).outputText;
const calls = [],
  timers = new Map(),
  listeners = new Map();
let timerId = 0,
  watcher,
  unmount,
  pendingPosition = null;
const store = { forceChoice: null, userName: "test", appendGameMessage() {} };
const windowMock = {
  innerWidth: 800,
  innerHeight: 600,
  setTimeout(fn, ms) {
    timers.set(++timerId, { fn, ms });
    return timerId;
  },
  addEventListener(name, fn) {
    listeners.set(name, fn);
  },
  removeEventListener(name) {
    listeners.delete(name);
  },
};
const documentMock = {
  visibilityState: "visible",
  addEventListener(name, fn) {
    listeners.set(name, fn);
  },
  removeEventListener(name) {
    listeners.delete(name);
  },
};
async function invoke(name, args) {
  calls.push({ name, args });
  if (name === "get_script_cursor_position") {
    if (pendingPosition) return pendingPosition;
    return { x: -500, y: 900 };
  }
}
const moduleMock = { exports: {} };
vm.runInNewContext(code, {
  exports: moduleMock.exports,
  module: moduleMock,
  require(name) {
    if (name === "vue")
      return {
        ref: (value) => ({ value }),
        nextTick: async () => {},
        watch: (_, cb) => {
          watcher = cb;
        },
        onMounted: (fn) => fn(),
        onBeforeUnmount: (fn) => {
          unmount = fn;
        },
      };
    if (name === "@tauri-apps/api/core") return { invoke };
    if (name === "@/stores/modules/game") return { useGameStore: () => store };
    throw new Error("Unexpected import: " + name);
  },
  window: windowMock,
  document: documentMock,
  console,
  clearTimeout: (id) => timers.delete(id),
});
const api = moduleMock.exports.test;
let panelRect = { left: 100, right: 700, top: 80, bottom: 460 };
const buttons = [0, 1, 2].map((i) => ({
  getBoundingClientRect: () => ({ left: 120, top: 100 + i * 100, width: 560, height: 50 }),
}));
api.attach(
  { querySelectorAll: () => ({ item: (i) => buttons[i] }) },
  { getBoundingClientRect: () => panelRect }
);
let request = 0;
async function start() {
  const fc = {
    requestId: "ticket-" + ++request,
    forced: "continue",
    choices: [{ text: "leave" }, { text: "back" }, { text: "continue" }],
  };
  store.forceChoice = fc;
  await watcher(fc);
  return fc;
}
const warps = () => calls.filter((c) => c.name === "warp_cursor");
async function stoppedBy(trigger) {
  await start();
  const generation = api.generation();
  trigger();
  const count = warps().length;
  await api.tick(generation);
  assert.equal(warps().length, count, "canceled generation must never move the cursor");
  assert.equal(timers.size, 0, "cancel must remove both timers");
}
(async () => {
  await start();
  await api.tick(api.generation());
  const first = warps().at(-1).args;
  assert(first.x >= 100 && first.x <= 700 && first.y >= 80 && first.y <= 460);
  assert.equal(first.x, 100);
  assert.equal(first.y, 460);
  listeners.get("keydown")({ key: "Escape" });
  await start();
  panelRect = { left: -50, right: 850, top: -100, bottom: 900 };
  await api.tick(api.generation());
  const clipped = warps().at(-1).args;
  assert(clipped.x >= 0 && clipped.x <= 799 && clipped.y >= 0 && clipped.y <= 599);
  // tick is normally invoked by the timer; clear its initial fake timer before cancellation cases.
  listeners.get("keydown")({ key: "Escape" });
  timers.clear();
  await stoppedBy(() => listeners.get("keydown")({ key: "Escape" }));
  await stoppedBy(() => listeners.get("blur")());
  await stoppedBy(() => {
    documentMock.visibilityState = "hidden";
    listeners.get("visibilitychange")();
    documentMock.visibilityState = "visible";
  });
  await stoppedBy(() => [...timers.values()].find((t) => t.ms === 5000).fn());
  const fc = await start();
  const count = calls.filter((c) => c.name === "script_submit_choice").length;
  await api.onChoiceClick(fc.choices[0]);
  assert.equal(calls.filter((c) => c.name === "script_submit_choice").length, count);
  await api.onChoiceClick(fc.choices[2]);
  assert.equal(calls.filter((c) => c.name === "script_submit_choice").length, count + 1);
  assert.equal(store.forceChoice, null);
  // A late cursor query must not restart a canceled/replaced event.
  let resolvePosition;
  pendingPosition = new Promise((resolve) => {
    resolvePosition = resolve;
  });
  const starting = start();
  await Promise.resolve();
  await Promise.resolve();
  listeners.get("blur")();
  resolvePosition({ x: 20, y: 20 });
  await starting;
  pendingPosition = null;
  assert.equal(timers.size, 0);
  await start();
  unmount();
  assert.equal(timers.size, 0);
  assert.equal(listeners.size, 0);
  console.log(
    "ForceChoice passed: panel/viewport bounds, Escape, blur, hidden, timeout, forced submission, stale query and unmount."
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
