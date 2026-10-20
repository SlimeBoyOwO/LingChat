// download_embedding_model.mjs
//
// 下载工具、笔记和聊天记忆共用的嵌入模型（bge-small-zh-v1.5，uint8 量化单文件 ONNX）。
// 结构与 download_emotion_model.mjs 保持一致：幂等、并行下载、失败退出非 0。
//
// 模型与 tokenizer 都不进 git（.gitignore 的 `data/**` 排除整个 data 目录），
// 靠本脚本获取，与情绪模型同一策略。
import { promises as fs } from "fs";
import path from "path";
import { createHash, randomUUID } from "crypto";
import readline from "readline";
import { pathToFileURL } from "url";

const TARGET_DIR = path.join(process.cwd(), "data", "third_party", "embedding_model_bge_small_zh");

// 远程路径 -> 本地文件名。统一落成扁平的 model.onnx，便于 Rust 侧按目录解析。
//
// 用 uint8 动态量化版（约 23MB）：同仓库还有 model_quantized / model_int8 / model_fp16 /
// model.onnx（fp32 约 90MB）等变体，换用哪个只改这里的 remote。量化会略微损失精度，
// 但工具检索是粗粒度匹配，实测区分度足够（见 tool_rag 的检索冒烟测试）。
const FILES = [
  { remote: "onnx/model_uint8.onnx", local: "model.onnx" },
  { remote: "tokenizer.json", local: "tokenizer.json" },
  { remote: "config.json", local: "config.json" },
];

// 换源只改这一处。ModelScope 国内直连稳定，与情绪模型同源。
const BASE_URL = "https://www.modelscope.cn/models/Xenova/bge-small-zh-v1.5/resolve/master";

// 是否启用交互式确认（可通过环境变量控制）
const INTERACTIVE = process.env.INTERACTIVE !== "false";

/** 计算文件的 SHA-256 哈希值 */
async function calculateSha256(filePath) {
  try {
    const fileBuffer = await fs.readFile(filePath);
    const hash = createHash("sha256");
    hash.update(fileBuffer);
    return hash.digest("hex");
  } catch (err) {
    if (err.code === "ENOENT") return null;
    throw err;
  }
}

/** 下载单个文件并返回内容 */
async function downloadFileContent(remote, fetchImpl) {
  const url = `${BASE_URL}/${remote}`;
  const response = await fetchImpl(url);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

/** 交互式确认：是否删除并重新下载 */
function askUserConfirmation(localName) {
  return new Promise((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    rl.question(
      `\n⚠️  ${localName} 内容与远程版本不一致。\n   是否重新下载并替换模型资源？(y/N) `,
      (answer) => {
        rl.close();
        resolve(answer.toLowerCase() === "y" || answer.toLowerCase() === "yes");
      },
    );
  });
}

/** 主函数 */
export async function downloadEmbeddingModel({
  targetDir = TARGET_DIR,
  fetchImpl = fetch,
  interactive = INTERACTIVE && process.stdin.isTTY,
  confirm = askUserConfirmation,
} = {}) {
  console.log(`📂 工作目录: ${process.cwd()}`);
  console.log(`📁 目标目录: ${targetDir}`);

  await fs.mkdir(targetDir, { recursive: true });
  // 已有资源先检查能否读取，空文件或损坏的 JSON 按缺失处理。
  const missing = [];
  for (const entry of FILES) {
    try {
      validateContent(entry.local, await fs.readFile(path.join(targetDir, entry.local)));
    } catch (error) {
      if (error.code && error.code !== "ENOENT") throw error;
      missing.push(entry);
    }
  }
  let requested = missing;
  let remoteConfigContent;
  if (missing.length === 0) {
    console.log(`🔍 正在下载远程 config.json 进行版本对比...`);
    try {
      remoteConfigContent = await downloadFileContent("config.json", fetchImpl);
      validateContent("config.json", remoteConfigContent);
    } catch (err) {
      console.error(`❌ 无法下载远程 config.json:`, err.message);
      console.log(`💡 跳过版本检查，保留现有文件。`);
      return;
    }

    const localSha = await calculateSha256(path.join(targetDir, "config.json"));
    const remoteSha = createHash("sha256").update(remoteConfigContent).digest("hex");

    if (localSha === remoteSha) {
      console.log(`✅ 本地 config.json 与远程版本一致，无需更新。`);
      return;
    }

    console.log(`🔄 检测到 config.json 版本不一致:`);
    console.log(`   本地 SHA: ${localSha}`);
    console.log(`   远程 SHA: ${remoteSha}`);

    if (interactive) {
      const shouldReDownload = await confirm("config.json");
      if (!shouldReDownload) {
        console.log(`⏭️  已跳过重新下载，保留现有所有文件。`);
        return;
      }
      requested = FILES;
    } else {
      console.log(`⏭️  非交互模式，跳过重新下载。`);
      console.log(`💡 如需强制重新下载，请设置环境变量 INTERACTIVE=true 或手动删除文件后重试。`);
      return;
    }
  } else {
    console.log(`📁 补齐 ${missing.length} 个缺失或无效文件。`);
  }

  console.log(`📥 开始并行下载 ${requested.length} 个文件...`);

  // 先下载到各自的临时文件，所有下载成功后才开始替换现有资源。
  const results = await Promise.allSettled(
    requested.map(async ({ remote, local }) => {
      const content =
        local === "config.json" && remoteConfigContent
          ? remoteConfigContent
          : await downloadFileContent(remote, fetchImpl);
      validateContent(local, content);
      const temporary = path.join(targetDir, `.${local}.${randomUUID()}.tmp`);
      try {
        await fs.writeFile(temporary, content, { flag: "wx" });
      } catch (error) {
        await fs.rm(temporary, { force: true });
        throw error;
      }
      return { local, temporary };
    }),
  );

  const failed = results.filter((r) => r.status === "rejected");
  const staged = results
    .filter((result) => result.status === "fulfilled")
    .map((result) => result.value);
  try {
    if (failed.length) {
      throw new AggregateError(
        failed.map((result) => result.reason),
        "模型资源下载未完成，未替换原文件",
      );
    }
    for (const entry of staged) {
      await fs.rename(entry.temporary, path.join(targetDir, entry.local));
      console.log(`✅ 已保存: ${entry.local}`);
    }
  } finally {
    // 下载失败或替换中断都清理临时文件，避免下次启动误读半成品。
    await Promise.all(staged.map((entry) => fs.rm(entry.temporary, { force: true })));
  }
  console.log(`🎉 所有必要文件已就绪，保存至: ${targetDir}`);
}

function validateContent(local, content) {
  if (content.length === 0) throw new Error(`${local} 为空`);
  if (local.endsWith(".json")) {
    const value = JSON.parse(content.toString("utf8"));
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      throw new Error(`${local} 必须是 JSON 对象`);
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  downloadEmbeddingModel().catch((error) => {
    console.error("❌ 脚本执行出错:", error);
    process.exitCode = 1;
  });
}
