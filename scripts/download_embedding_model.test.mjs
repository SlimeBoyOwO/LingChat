import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { downloadEmbeddingModel } from "./download_embedding_model.mjs";

const resources = {
  "model.onnx": Buffer.from("synthetic-onnx-model"),
  "tokenizer.json": Buffer.from('{"model":{"type":"WordPiece"}}'),
  "config.json": Buffer.from('{"model_type":"bert"}'),
};

async function fixture(context, excluded = []) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "lingchat-embedding-test-"));
  context.after(async () => {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(directory).startsWith("lingchat-embedding-test-"));
    await fs.rm(directory, { recursive: true, force: true });
  });
  for (const [name, content] of Object.entries(resources)) {
    if (!excluded.includes(name)) await fs.writeFile(path.join(directory, name), content);
  }
  return directory;
}

function remote(calls, overrides = {}) {
  return async (url) => {
    const name = url.endsWith("model_uint8.onnx") ? "model.onnx" : path.basename(url);
    calls.push(name);
    const content = Object.hasOwn(overrides, name) ? overrides[name] : resources[name];
    return {
      ok: true,
      arrayBuffer: async () => {
        if (content instanceof Error) throw content;
        return content;
      },
    };
  };
}

test("fresh directory downloads every required file", async (context) => {
  const directory = await fixture(context, Object.keys(resources));
  const calls = [];
  await downloadEmbeddingModel({
    targetDir: directory,
    fetchImpl: remote(calls),
    interactive: false,
  });
  assert.deepEqual(calls.sort(), Object.keys(resources).sort());
  for (const [name, content] of Object.entries(resources)) {
    assert.deepEqual(await fs.readFile(path.join(directory, name)), content);
  }
  assert.deepEqual((await fs.readdir(directory)).sort(), Object.keys(resources).sort());
});

for (const missing of Object.keys(resources)) {
  test(`partial directory repairs only missing ${missing}`, async (context) => {
    const directory = await fixture(context, [missing]);
    const calls = [];
    await downloadEmbeddingModel({
      targetDir: directory,
      fetchImpl: remote(calls),
      interactive: false,
    });
    assert.deepEqual(calls, [missing]);
    assert.deepEqual(await fs.readFile(path.join(directory, missing)), resources[missing]);
  });
}

test("complete matching directory preserves files without redundant downloads", async (context) => {
  const directory = await fixture(context);
  const calls = [];
  await downloadEmbeddingModel({
    targetDir: directory,
    fetchImpl: remote(calls),
    interactive: false,
  });
  assert.deepEqual(calls, ["config.json"]);
  assert.deepEqual(await fs.readFile(path.join(directory, "model.onnx")), resources["model.onnx"]);
});

test("failed refresh leaves every original file intact and removes temporary files", async (context) => {
  const directory = await fixture(context);
  const overrides = {
    "config.json": Buffer.from('{"model_type":"bert","revision":2}'),
    "model.onnx": Buffer.from("new-model"),
    "tokenizer.json": new Error("interrupted download"),
  };
  await assert.rejects(
    downloadEmbeddingModel({
      targetDir: directory,
      fetchImpl: remote([], overrides),
      interactive: true,
      confirm: async () => true,
    }),
    AggregateError,
  );
  for (const [name, content] of Object.entries(resources)) {
    assert.deepEqual(await fs.readFile(path.join(directory, name)), content);
  }
  assert.deepEqual((await fs.readdir(directory)).sort(), Object.keys(resources).sort());
});

test("failed first download can be retried successfully", async (context) => {
  const directory = await fixture(context, Object.keys(resources));
  await assert.rejects(
    downloadEmbeddingModel({
      targetDir: directory,
      fetchImpl: remote([], { "tokenizer.json": new Error("interrupted") }),
      interactive: false,
    }),
    AggregateError,
  );
  assert.deepEqual(await fs.readdir(directory), []);
  await downloadEmbeddingModel({ targetDir: directory, fetchImpl: remote([]), interactive: false });
  assert.deepEqual((await fs.readdir(directory)).sort(), Object.keys(resources).sort());
});

test("empty or malformed local resources are repaired", async (context) => {
  const directory = await fixture(context);
  await fs.writeFile(path.join(directory, "model.onnx"), "");
  await fs.writeFile(path.join(directory, "tokenizer.json"), "invalid json");
  const calls = [];
  await downloadEmbeddingModel({
    targetDir: directory,
    fetchImpl: remote(calls),
    interactive: false,
  });
  assert.deepEqual(calls.sort(), ["model.onnx", "tokenizer.json"]);
});

test("invalid remote JSON never becomes a final resource file", async (context) => {
  const directory = await fixture(context, ["tokenizer.json"]);
  await assert.rejects(
    downloadEmbeddingModel({
      targetDir: directory,
      fetchImpl: remote([], { "tokenizer.json": Buffer.from("<html>error</html>") }),
      interactive: false,
    }),
    AggregateError,
  );
  assert.deepEqual((await fs.readdir(directory)).sort(), ["config.json", "model.onnx"]);
});

test("successful refresh replaces only known model resources", async (context) => {
  const directory = await fixture(context);
  await fs.writeFile(path.join(directory, "README.md"), "keep this documentation");
  const replacements = {
    "config.json": Buffer.from('{"model_type":"bert","revision":2}'),
    "model.onnx": Buffer.from("new-model"),
    "tokenizer.json": Buffer.from('{"model":{"type":"WordPiece"},"revision":2}'),
  };
  await downloadEmbeddingModel({
    targetDir: directory,
    fetchImpl: remote([], replacements),
    interactive: true,
    confirm: async () => true,
  });
  for (const [name, content] of Object.entries(replacements)) {
    assert.deepEqual(await fs.readFile(path.join(directory, name)), content);
  }
  assert.equal(
    await fs.readFile(path.join(directory, "README.md"), "utf8"),
    "keep this documentation",
  );
});
