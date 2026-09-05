#!/usr/bin/env node
// Run actual TS queue/tool handlers with deterministic processor/timer stubs.
// No framework/install required. TYPESCRIPT_PATH optionally points at an existing install.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
const require = createRequire(import.meta.url);
const ts = process.env.TYPESCRIPT_PATH
  ? require(process.env.TYPESCRIPT_PATH)
  : require("typescript");
const root = new URL("../../", import.meta.url);
const game = { currentInteractRoleId: 1 };
const ui = { autoMode: false };
let processor;
const stubs = {
  "../../stores/modules/game": { useGameStore: () => game },
  "../../stores/modules/settings": {
    useSettingsStore: () => ({ text: { inlineMotionText: false } }),
  },
  "../../stores/modules/ui/ui": { useUIStore: () => ui },
  "./dialogue-merge": { dialogueMerge: {} },
  "./event-processor": { eventProcessorManager: { processEvent: (e) => processor(e) } },
  "@tauri-apps/api/core": { invoke: async () => undefined },
  vue: { ref: (value) => ({ value }) },
  "@/locales": { i18n: { global: { t: (key) => key, te: () => false } } },
};
function load(path) {
  const filename = fileURLToPath(new URL(path, root));
  const { outputText, diagnostics } = ts.transpileModule(readFileSync(filename, "utf8"), {
    fileName: filename,
    reportDiagnostics: true,
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  assert.equal(diagnostics?.filter((d) => d.category === ts.DiagnosticCategory.Error).length, 0);
  const exports = {};
  vm.runInNewContext(
    outputText,
    {
      exports,
      require: (id) => {
        assert.ok(id in stubs, `unmocked import ${id}`);
        return stubs[id];
      },
      console: { log() {}, error() {}, warn() {} },
      setTimeout: () => 1,
      clearTimeout() {},
    },
    { filename },
  );
  return exports;
}
const { EventQueue } = load("src/core/events/event-queue.ts");
const tick = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};
const deferred = () => {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
};
const reply = (text) => ({ type: "reply", message: text, roleId: 1, duration: -1, isFinal: false });

// Last queued reply must not be resolved by an earlier reply's async processor.
{
  const queue = new EventQueue();
  const gates = [deferred(), deferred()];
  let i = 0;
  processor = () => gates[i++].promise;
  queue.resume();
  queue.addEvent(reply("first"));
  const first = queue.waitForLatestReplyPresentation();
  queue.addEvent(reply("preamble"));
  const last = queue.waitForLatestReplyPresentation();
  let state = "pending";
  last.then((v) => (state = v));
  gates[0].resolve();
  await tick();
  assert.equal(await first, true);
  assert.equal(state, "pending");
  queue.continue();
  await tick();
  gates[1].resolve();
  await tick();
  assert.equal(await last, true);
  assert.equal(queue.getState().isWaitingForUser, true, "presentation must not wait for a click");
  queue.clear();
}

// Tool start, result notification, finish all wait for the SAME preamble even
// when a suffix is queued immediately. Clear cancels without resurrection.
for (const cancel of [false, true]) {
  const queue = new EventQueue();
  stubs["../../core/events/event-queue"] = { eventQueue: queue };
  const tools = load("src/api/services/tool-settings.ts");
  const gate = deferred();
  processor = () => gate.promise;
  queue.resume();
  queue.addEvent(reply("preamble"));
  const event = {
    call_id: "call-1",
    tool: "read_file",
    arguments: "{}",
    phase: "started",
    wait_for_reply: true,
  };
  tools.handleToolActivity(event);
  let notified = false;
  tools.afterToolPresentation("call-1", () => (notified = true));
  tools.handleToolActivity({ ...event, phase: "finished", wait_for_reply: false, ok: true });
  queue.addEvent(reply("suffix"));
  await tick();
  assert.equal(notified, false);
  if (cancel) queue.clear();
  gate.resolve();
  await tick();
  assert.equal(notified, !cancel);
  if (cancel) {
    tools.afterToolPresentation("call-1", () => {
      throw Error("late cancelled result resurrected");
    });
  } else {
    assert.equal(tools.currentToolActivity.value.status, "success");
    assert.equal(queue.getState().isWaitingForUser, true);
  }
  queue.clear();
  tools.interruptToolActivities();
}

// Paused queue and dropped preview cannot accidentally acknowledge another reply.
{
  const queue = new EventQueue();
  queue.addEvent(reply("paused"));
  const pending = queue.waitForLatestReplyPresentation();
  queue.discardReplyPresentation();
  assert.equal(await queue.waitForLatestReplyPresentation(), false);
  queue.clear();
  assert.equal(await pending, false);
}
// Clearing an active wait or an awaited processor cannot create a zombie
// consumer that later acknowledges/reset a new generation's reply.
for (const duringProcessor of [false, true]) {
  const queue = new EventQueue();
  const old = deferred();
  const current = deferred();
  let index = 0;
  processor = () => (index++ === 0 ? old.promise : current.promise);
  queue.resume();
  queue.addEvent(reply("old"));
  if (!duringProcessor) {
    old.resolve();
    await tick();
  }
  queue.clear();
  queue.resume();
  queue.addEvent(reply("new"));
  const presented = queue.waitForLatestReplyPresentation();
  old.resolve();
  await tick();
  assert.equal(queue.getState().isWaitingForUser, false);
  assert.equal(queue.getState().isProcessing, true);
  current.resolve();
  await tick();
  assert.equal(await presented, true);
  assert.equal(queue.getState().isWaitingForUser, true);
  queue.clear();
  await tick();
  assert.equal(queue.getState().isProcessing, false);
  assert.equal(queue.getState().isWaitingForUser, false);
}
console.log("tool-order: 7 production queue/handler cases passed");
