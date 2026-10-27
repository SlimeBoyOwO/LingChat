// download_embedding_model.mjs
//
// 下载 Tool RAG 用的文本嵌入模型（bge-small-zh-v1.5，uint8 量化单文件 ONNX）。
// 结构与 download_emotion_model.mjs 保持一致：幂等、并行下载、失败退出非 0。
//
// 模型与 tokenizer 都不进 git（.gitignore 的 `data/**` 排除整个 data 目录），
// 靠本脚本获取，与情绪模型同一策略。
//
// 由 `pnpm run init` 调用。与 ensure-onnxruntime.mjs 一样不要挂到
// beforeDevCommand/beforeBuildCommand，避免每次启动都做资源检查。
import { promises as fs } from "fs";
import path from "path";
import { createHash } from "crypto";
import readline from "readline";

const TARGET_DIR = path.join(
  process.cwd(),
  "data",
  "third_party",
  "embedding_model_bge_small_zh",
);

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
const BASE_URL =
  "https://www.modelscope.cn/models/Xenova/bge-small-zh-v1.5/resolve/master";

// 是否启用交互式确认（可通过环境变量控制）
const INTERACTIVE = process.env.INTERACTIVE !== "false";

/** 文件是否存在 */
async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

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
async function downloadFileContent(remote) {
  const url = `${BASE_URL}/${remote}`;
  const response = await fetch(url);
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
      `\n⚠️  ${localName} 内容与远程版本不一致。\n   是否删除本地文件并重新下载？(y/N) `,
      (answer) => {
        rl.close();
        resolve(answer.toLowerCase() === "y" || answer.toLowerCase() === "yes");
      },
    );
  });
}

/** 主函数 */
async function main() {
  console.log(`📂 工作目录: ${process.cwd()}`);
  console.log(`📁 目标目录: ${TARGET_DIR}`);

  await fs.mkdir(TARGET_DIR, { recursive: true });

  // 逐个检查，半途中断留下的残缺目录也能补齐
  const missing = [];
  for (const file of FILES) {
    if (!(await fileExists(path.join(TARGET_DIR, file.local)))) {
      missing.push(file);
    }
  }

  if (missing.length === 0) {
    console.log(`✅ 目录中已存在全部分件`);

    console.log(`🔍 正在下载远程 config.json 进行版本对比...`);
    let remoteConfigContent;
    try {
      remoteConfigContent = await downloadFileContent("config.json");
    } catch (err) {
      console.error(`❌ 无法下载远程 config.json:`, err.message);
      console.log(`💡 跳过版本检查，保留现有文件。`);
      return;
    }

    const localSha = await calculateSha256(path.join(TARGET_DIR, "config.json"));
    const remoteSha = createHash("sha256")
      .update(remoteConfigContent)
      .digest("hex");

    if (localSha === remoteSha) {
      console.log(`✅ 本地 config.json 与远程版本一致，无需更新。`);
      return;
    }

    console.log(`🔄 检测到 config.json 版本不一致:`);
    console.log(`   本地 SHA: ${localSha}`);
    console.log(`   远程 SHA: ${remoteSha}`);

    if (INTERACTIVE && process.stdin.isTTY) {
      const shouldReDownload = await askUserConfirmation("config.json");
      if (!shouldReDownload) {
        console.log(`⏭️  已跳过重新下载，保留现有所有文件。`);
        return;
      }
      console.log(`🗑️  正在删除现有文件...`);
      for (const entry of await fs.readdir(TARGET_DIR)) {
        const entryPath = path.join(TARGET_DIR, entry);
        if ((await fs.stat(entryPath)).isFile()) {
          await fs.unlink(entryPath);
          console.log(`  已删除: ${entry}`);
        }
      }
      console.log(`✅ 清理完成，开始重新下载所有文件...`);
    } else {
      console.log(`⏭️  非交互模式，跳过重新下载。`);
      console.log(
        `💡 如需强制重新下载，请设置环境变量 INTERACTIVE=true 或手动删除文件后重试。`,
      );
      return;
    }
  } else {
    console.log(`📁 缺少 ${missing.length}/${FILES.length} 个文件，开始下载...`);
  }

  const pending = missing.length === 0 ? FILES : missing;
  console.log(`📥 开始并行下载 ${pending.length} 个文件...`);

  const results = await Promise.allSettled(
    pending.map(async ({ remote, local }) => {
      const content = await downloadFileContent(remote);
      await fs.writeFile(path.join(TARGET_DIR, local), content);
      console.log(`✅ 已下载: ${local} (${remote})`);
      return local;
    }),
  );

  const failed = results.filter((r) => r.status === "rejected");
  for (const f of failed) {
    console.error(`❌ 下载失败:`, f.reason?.message ?? f.reason);
  }

  if (failed.length === 0) {
    console.log(
      `🎉 所有 ${results.length} 个文件下载完成，保存至: ${TARGET_DIR}`,
    );
  } else {
    console.log(
      `⚠️ 下载完成: ${results.length - failed.length} 个成功，${failed.length} 个失败`,
    );
    process.exit(1);
  }
}

main().catch((err) => {
  console.error("❌ 脚本执行出错:", err);
  process.exit(1);
});
