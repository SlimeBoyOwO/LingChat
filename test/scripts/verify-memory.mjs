#!/usr/bin/env node
/** Offline memory-test-api verification with a failure-safe, atomic report. */
import { createHash, randomUUID } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const artifacts = join(root, "test", "artifacts");
mkdirSync(artifacts, { recursive: true });
const reportPath = join(artifacts, "memory-report.json");
const startedAt = new Date().toISOString();
const runId = randomUUID();
const commit = (() => {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "unknown";
})();
const fixturesDir = join(root, "test", "fixtures", "memory");
const fixtureNames = ["basic.json", "append_during_update.json", "partial_failure.json", "panic.json", "rollback.json", "multilingual.json"];
const commandResults = [];
// Keep report creation ahead of fixture execution: even a missing/corrupt
// fixture must leave an explicit failed artifact instead of an old success.
const fixtureHashes = Object.fromEntries(fixtureNames.map((name) => {
  try {
    const bytes = readFileSync(join(fixturesDir, name));
    return [name, createHash("sha256").update(bytes).digest("hex")];
  } catch {
    return [name, "unavailable"];
  }
}));
const atomicWrite = (value) => {
  const temp = `${reportPath}.${runId}.tmp`;
  writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`);
  try {
    renameSync(temp, reportPath);
  } catch (error) {
    if (process.platform !== "win32" || !existsSync(reportPath)) throw error;
    // Node's Windows rename cannot replace an existing destination. Move the
    // old ignored artifact aside, install the prepared file, and restore the
    // old one if installation fails.
    const previous = `${reportPath}.${runId}.previous`;
    renameSync(reportPath, previous);
    try {
      renameSync(temp, reportPath);
      unlinkSync(previous);
    } catch (installError) {
      try { renameSync(previous, reportPath); } catch {}
      throw installError;
    }
  }
};
const baseReport = {
  ok: false, status: "running", run_id: runId, commit_sha: commit,
  started_at: startedAt, fixture_hashes: fixtureHashes, scenarios: [],
};
atomicWrite(baseReport);

const cargo = process.env.CARGO ?? (process.platform === "win32" ? "cargo.exe" : "cargo");
const manifest = join(root, "src-tauri", "Cargo.toml");
const cargoEnv = { ...process.env };
if (process.platform === "win32" && !cargoEnv.RUSTUP_TOOLCHAIN) {
  cargoEnv.RUSTUP_TOOLCHAIN = "stable";
  cargoEnv.RUSTC ??= "D:/DevEnvs/Rust/.rustup/toolchains/stable-x86_64-pc-windows-msvc/bin/rustc.exe";
  cargoEnv.PATH = `D:/DevEnvs/Rust/.rustup/toolchains/stable-x86_64-pc-windows-msvc/bin;D:/DevEnvs/Rust/.cargo/bin;${cargoEnv.PATH ?? ""}`;
}
const cargoArgs = ["--manifest-path", manifest, "--features", "memory-test-api", "--bin", "memory-test-api"];
const run = (args) => {
  const result = spawnSync(cargo, args, { cwd: root, stdio: "inherit", env: cargoEnv });
  commandResults.push({ command: `${cargo} ${args.join(" ")}`, exit_code: result.status ?? 1 });
  if (result.status !== 0) throw new Error(`cargo ${args[0]} failed (exit ${result.status ?? "unknown"})`);
};
const finishFailure = (error, exitCode = 1) => {
  const finishedAt = new Date().toISOString();
  atomicWrite({ ...baseReport, status: "failed", ok: false, finished_at: finishedAt,
    exit_code: exitCode, error: String(error?.stack ?? error), commands: commandResults, scenarios: results });
  console.error(error);
  return exitCode;
};
let child;
let shutdownUrl;
let shutdownHeaders;
const results = [];
try {
  const prepare = spawnSync(process.execPath, [join(root, "scripts", "prepare-desktop-resources.mjs")], { cwd: root, stdio: "inherit" });
  commandResults.push({ command: `${process.execPath} scripts/prepare-desktop-resources.mjs`, exit_code: prepare.status ?? 1 });
  if (prepare.status !== 0) throw new Error(`resource preparation failed (exit ${prepare.status ?? "unknown"})`);
  run(["test", ...cargoArgs, "--lib"]);
  run(["build", ...cargoArgs]);
  const binary = join(root, "src-tauri", "target", "debug", process.platform === "win32" ? "memory-test-api.exe" : "memory-test-api");
  if (!existsSync(binary)) throw new Error(`memory-test-api binary not found: ${binary}`);
  child = spawn(binary, [], { cwd: root, stdio: ["ignore", "pipe", "inherit"], env: cargoEnv });
  let buffer = "";
  const ready = await new Promise((resolveReady, reject) => {
    const timer = setTimeout(() => reject(new Error("memory-test-api ready timeout")), 30_000);
    child.stdout.on("data", (chunk) => {
      buffer += chunk;
      const newline = buffer.indexOf("\n");
      if (newline < 0) return;
      clearTimeout(timer);
      try { resolveReady(JSON.parse(buffer.slice(0, newline).trim())); } catch (error) { reject(error); }
    });
    child.once("error", reject);
  });
  if (ready.host !== "127.0.0.1" || !ready.port || !ready.token) throw new Error("invalid ready response");
  const base = `http://${ready.host}:${ready.port}`;
  const auth = { Authorization: `Bearer ${ready.token}` };
  shutdownUrl = `${base}/shutdown`;
  shutdownHeaders = auth;
  const scenarioNames = ["basic-compression", "append-during-update", "one-section-fails", "empty-section-fails", "panic-compression", "stale-on-rollback", "persistence-roundtrip", "memory-finishes-after-line-save"];
  const fixtureFor = {
    "basic-compression": "basic.json", "append-during-update": "append_during_update.json",
    "one-section-fails": "partial_failure.json", "empty-section-fails": "partial_failure.json",
    "panic-compression": "panic.json", "stale-on-rollback": "rollback.json",
    "persistence-roundtrip": "basic.json",
    "memory-finishes-after-line-save": "basic.json",
  };
  for (const name of scenarioNames) {
    const fixture = fixtureFor[name];
    const payload = JSON.parse(readFileSync(join(fixturesDir, fixture), "utf8"));
    payload.scenario = name;
    const response = await fetch(`${base}/v1/scenarios/${name}`, {
      method: "POST", headers: { ...auth, "content-type": "application/json" }, body: JSON.stringify(payload),
    });
    const body = await response.json();
    if (response.status !== 200) throw new Error(`${name}: HTTP ${response.status} ${JSON.stringify(body)}`);
    if (body.committed && body.outcome !== "succeeded") throw new Error(`${name}: committed outcome mismatch`);
    if (name === "basic-compression" && (body.outcome !== "succeeded" || body.calls !== 4 || !body.triggered)) throw new Error(`${name}: assertion failed`);
    if (["one-section-fails", "empty-section-fails", "stale-on-rollback"].includes(name) && (body.committed || body.outcome !== "not_committed" || body.calls !== 4 || body.last_processed_global_idx !== 0)) throw new Error(`${name}: rollback assertion failed`);
    if (name === "panic-compression" && (body.outcome !== "not_committed" || body.committed || body.calls !== 4 || body.last_processed_global_idx !== 0 || !body.triggered)) throw new Error(`${name}: panic rollback assertion failed`);
    if (name === "append-during-update" && (body.outcome !== "succeeded" || body.first_processed_global_idx !== 4 || body.unprocessed_tail_lines !== 1 || !body.second_batch_committed || body.calls !== 8)) throw new Error(`${name}: append assertion failed`);
    if (name === "persistence-roundtrip" && body.persistence_roundtrip !== true) throw new Error(`${name}: round-trip assertion failed`);
    if (name === "memory-finishes-after-line-save" && (body.outcome !== "succeeded" || body.persistence_roundtrip !== true || body.details?.persisted_last_processed_global_idx !== 2)) throw new Error(`${name}: late autosave assertion failed`);
    results.push({ name, fixture, fixture_sha256: fixtureHashes[fixture], status: response.status, outcome: body.outcome, calls: body.calls, committed: body.committed, assertions: { triggered: body.triggered, pointer: body.last_processed_global_idx, tail: body.unprocessed_tail_lines, persistence_roundtrip: body.persistence_roundtrip, persisted_last_processed_global_idx: body.details?.persisted_last_processed_global_idx } });
  }
  const multilingual = JSON.parse(readFileSync(join(fixturesDir, "multilingual.json"), "utf8"));
  const contractResponse = await fetch(`${base}/v1/memory/validate`, { method: "POST", headers: { ...auth, "content-type": "application/json" }, body: JSON.stringify(multilingual) });
  if (contractResponse.status !== 200) throw new Error(`multilingual fixture contract HTTP ${contractResponse.status}`);
  const contract = await contractResponse.json();
  results.push({ name: "fixture-contract:multilingual", fixture: "multilingual.json", fixture_sha256: fixtureHashes["multilingual.json"], status: contractResponse.status, assertions: { utf8: true, display_name: multilingual.display_name, committed: contract.committed } });
  const unauthorized = await fetch(`${base}/health`);
  if (unauthorized.status !== 401) throw new Error(`health auth status ${unauthorized.status}`);
  atomicWrite({ ...baseReport, ok: true, status: "passed", finished_at: new Date().toISOString(), exit_code: 0, api_version: ready.api_version, commands: commandResults, scenarios: results });
  console.log(JSON.stringify({ ok: true, run_id: runId, scenarios: results.map(({ name }) => name) }));
} catch (error) {
  process.exitCode = finishFailure(error);
} finally {
  if (child) {
    try {
      if (shutdownUrl) await fetch(shutdownUrl, { method: "POST", headers: shutdownHeaders }).catch(() => {});
      // If startup failed, killing the child is the only safe cleanup.
      if (child.exitCode === null && !shutdownUrl) child.kill();
    } catch {}
    await new Promise((resolveExit) => {
      if (child.exitCode !== null) return resolveExit();
      const timer = setTimeout(() => { child.kill(); resolveExit(); }, 5_000);
      child.once("exit", () => { clearTimeout(timer); resolveExit(); });
    });
  }
}
