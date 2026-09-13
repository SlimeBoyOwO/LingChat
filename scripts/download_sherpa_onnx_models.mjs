#!/usr/bin/env node

/**
 * Sherpa-ONNX 模型下载脚本
 *
 * 用法:
 *   node scripts/download_sherpa_onnx_models.mjs [model-name] [language]
 *
 * 示例:
 *   node scripts/download_sherpa_onnx_models.mjs vits zh
 *   node scripts/download_sherpa_onnx_models.mjs fastspeech2 en
 */

import fs from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

// 获取当前脚本目录
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// 模型配置
const MODEL_CONFIGS = {
  "vits-zh": {
    name: "VITS Chinese",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-vits-zh-aishell3.tar.bz2",
    model_type: "vits",
    language: "zh",
    voice: "female",
    description: "中文 VITS 模型",
  },
  "vits-en": {
    name: "VITS English",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-vits-en-ljspeech.tar.bz2",
    model_type: "vits",
    language: "en",
    voice: "female",
    description: "英文 VITS 模型",
  },
  "fastspeech2-zh": {
    name: "FastSpeech2 Chinese",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-fastspeech2-zh-aishell3.tar.bz2",
    model_type: "fastspeech2",
    language: "zh",
    voice: "female",
    description: "中文 FastSpeech2 模型",
  },
  "fastspeech2-en": {
    name: "FastSpeech2 English",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-fastspeech2-en-ljspeech.tar.bz2",
    model_type: "fastspeech2",
    language: "en",
    voice: "female",
    description: "英文 FastSpeech2 模型",
  },
  "kokoro": {
    name: "Kokoro 多语言",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-kokoro-multi-lang-v1_1.tar.bz2",
    model_type: "kokoro",
    language: "zh",
    voice: "female",
    description: "Kokoro 多语言模型",
  },
  "zipvoice": {
    name: "ZipVoice 中英 (Emilia)",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-zipvoice-distill-zh-en-emilia.tar.bz2",
    model_type: "zipvoice",
    language: "zh",
    voice: "female",
    description: "ZipVoice 中英零样本克隆模型",
  },
  "matcha-zh": {
    name: "Matcha Chinese",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-matcha-tts-zh.tar.bz2",
    model_type: "matcha",
    language: "zh",
    voice: "female",
    description: "中文 Matcha-TTS 模型",
  },
  "matcha-en": {
    name: "Matcha English",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-matcha-tts-en.tar.bz2",
    model_type: "matcha",
    language: "en",
    voice: "female",
    description: "英文 Matcha-TTS 模型",
  },
};

// 模型存储目录（与运行时保持一致：app 固定读取 data/sherpa_onnx_models/）
const MODEL_DIR = path.join(process.cwd(), "data", "sherpa_onnx_models");

// 显示帮助信息
function showHelp() {
  console.log("Sherpa-ONNX 模型下载脚本\n");
  console.log("用法:");
  console.log("  node scripts/download_sherpa_onnx_models.mjs [model-name] [language]\n");
  console.log("可用模型:");
  Object.entries(MODEL_CONFIGS).forEach(([key, config]) => {
    console.log(`  ${key.padEnd(20)} ${config.description}`);
  });
  console.log("\n示例:");
  console.log("  node scripts/download_sherpa_onnx_models.mjs vits-zh");
  console.log("  node scripts/download_sherpa_onnx_models.mjs fastspeech2 en");
  process.exit(0);
}

// 显示模型列表
function showModelList() {
  console.log("可用模型:\n");
  console.log("模型名称".padEnd(20) + "描述");
  console.log("-".repeat(50));
  Object.entries(MODEL_CONFIGS).forEach(([key, config]) => {
    console.log(`${key.padEnd(20)} ${config.description}`);
  });
  process.exit(0);
}

// 下载文件
async function downloadFile(url, outputPath) {
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const contentLength = response.headers.get("content-length");
    const total = parseInt(contentLength || "0", 10);

    const buffer = await response.arrayBuffer();
    await fs.writeFile(outputPath, Buffer.from(buffer));

    console.log(`✓ 下载完成: ${outputPath}`);
    console.log(`  大小: ${(buffer.byteLength / 1024 / 1024).toFixed(2)} MB`);

    return buffer.byteLength;
  } catch (error) {
    throw new Error(`下载失败: ${error.message}`);
  }
}

// 解压 tar.bz2 并把模型文件规整到目标模型目录（modelDir）
//
// sherpa-onnx 发布的 tarball 解出来通常是一个子目录（如 `sherpa-onnx-vits-zh-aishell3/`），
// 运行时要求模型文件直接位于 `<models>/<modelKey>/` 下，因此这里把该顶层目录重命名为 modelKey。
async function extractToModelDir(tempFile, modelDir) {
  try {
    console.log(`📦 解压文件: ${tempFile}`);

    const { exec } = await import("child_process");
    const { promisify } = await import("util");
    const execAsync = promisify(exec);

    const extractDir = path.join(modelDir, ".extract_tmp");
    await fs.rm(extractDir, { recursive: true, force: true });
    await fs.mkdir(extractDir, { recursive: true });

    await execAsync(`7z x "${tempFile}" -o"${extractDir}" -y`);

    const entries = await fs.readdir(extractDir, { withFileTypes: true });
    const subDirs = entries.filter((entry) => entry.isDirectory());
    const topFiles = entries.filter((entry) => entry.isFile());

    await fs.mkdir(path.dirname(modelDir), { recursive: true });
    await fs.rm(modelDir, { recursive: true, force: true });

    if (subDirs.length === 1 && topFiles.length === 0) {
      // 单顶层目录：整体重命名，保证模型文件位于 modelDir 根下
      await fs.rename(path.join(extractDir, subDirs[0].name), modelDir);
    } else {
      await fs.rename(extractDir, modelDir);
    }

    console.log(`✓ 解压完成: ${modelDir}`);
  } catch (error) {
    throw new Error(`解压失败: ${error.message}`);
  }
}

// 验证下载的模型（依赖已写入的 config.json；按 model_type 检查对应的关键文件）
async function validateModel(modelDir, modelType) {
  const configPath = path.join(modelDir, "config.json");
  try {
    await fs.access(configPath);
  } catch {
    throw new Error("模型缺少 config.json（应由脚本自动生成，请确认脚本版本）");
  }

  const modelFilesByType = {
    vits: ["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"],
    fastspeech2: ["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"],
    matcha: ["model.onnx", "model-steps-3.onnx", "model-steps-6.onnx"],
    kokoro: ["model.onnx", "model.int8.onnx"],
    kitten: ["model.onnx", "model.int8.onnx"],
    zipvoice: ["fm_decoder.onnx", "text_encoder.onnx"],
    pocket: ["lm_flow.int8.onnx", "lm_flow.onnx"],
    supertonic: ["tts.json"],
  };
  const candidates = modelFilesByType[modelType] || modelFilesByType.vits;

  let found = null;
  for (const name of candidates) {
    try {
      await fs.access(path.join(modelDir, name));
      found = name;
      break;
    } catch {
      // 继续尝试下一个候选文件
    }
  }

  if (!found) {
    throw new Error(
      `模型文件不完整（model_type=${modelType}），未找到: ${candidates.join(" / ")}`
    );
  }

  const stats = await fs.stat(path.join(modelDir, found));
  if (stats.size < 1024 * 1024) {
    // 小于 1MB 可能有问题
    throw new Error(`模型文件大小异常: ${(stats.size / 1024 / 1024).toFixed(2)} MB`);
  }

  console.log(`✓ 模型验证通过: ${modelDir}`);
}

// 下载模型
async function downloadModel(modelKey) {
  if (!MODEL_CONFIGS[modelKey]) {
    throw new Error(`未知模型: ${modelKey}`);
  }

  const config = MODEL_CONFIGS[modelKey];
  const modelDir = path.join(MODEL_DIR, modelKey);
  const tempDir = path.join(MODEL_DIR, ".download_tmp");
  const tempFile = path.join(tempDir, `${modelKey}.tar.bz2`);

  try {
    console.log(`🚀 开始下载模型: ${config.name}`);
    console.log(`📋 描述: ${config.description}`);
    console.log(`🔗 URL: ${config.url}`);
    console.log(`📂 目标目录: ${modelDir}\n`);

    // 创建临时目录
    await fs.mkdir(tempDir, { recursive: true });

    // 下载文件
    console.log("⬇️ 下载中...");
    const downloadedSize = await downloadFile(config.url, tempFile);

    // 解压并规整到模型目录
    console.log("\n📦 解压中...");
    await extractToModelDir(tempFile, modelDir);

    // 先写入配置文件（运行时识别模型类型依赖它，验证也需要它）
    const configPath = path.join(modelDir, "config.json");
    const configContent = JSON.stringify(
      {
        model_type: config.model_type,
        language: config.language,
        voice: config.voice,
        name: config.name,
        downloaded_at: new Date().toISOString(),
        source_url: config.url,
        size_bytes: downloadedSize,
      },
      null,
      2
    );

    await fs.writeFile(configPath, configContent);

    // 验证模型
    console.log("\n✅ 验证中...");
    await validateModel(modelDir, config.model_type);

    // 清理临时文件
    await fs.rm(tempDir, { recursive: true, force: true });

    console.log("\n🎉 模型下载完成!");
    console.log(`📁 模型目录: ${modelDir}`);
    console.log(`📄 配置文件: ${configPath}`);
    console.log(`🔧 模型类型: ${config.model_type}`);
    console.log(`🌐 语言: ${config.language}`);
    console.log(`🎤 音色: ${config.voice}`);
  } catch (error) {
    console.error(`❌ 模型下载失败: ${error.message}`);
    process.exit(1);
  }
}

// 列出已下载的模型
async function listDownloadedModels() {
  try {
    await fs.access(MODEL_DIR);
  } catch {
    console.log("📂 模型目录不存在，没有已下载的模型");
    return;
  }

  const entries = await fs.readdir(MODEL_DIR, { withFileTypes: true });
  const modelDirs = entries.filter((entry) => entry.isDirectory());

  if (modelDirs.length === 0) {
    console.log("📂 模型目录为空，没有已下载的模型");
    return;
  }

  console.log("已下载的模型:\n");
  console.log(
    "模型名称".padEnd(20) + "类型".padEnd(12) + "语言".padEnd(8) + "音色".padEnd(12) + "大小"
  );
  console.log("-".repeat(60));

  for (const entry of modelDirs) {
    const modelDir = path.join(MODEL_DIR, entry.name);
    const configPath = path.join(modelDir, "config.json");

    try {
      const configContent = await fs.readFile(configPath, "utf-8");
      const config = JSON.parse(configContent);

      const entries = await fs.readdir(modelDir, { withFileTypes: true });
      const onnx = entries.find((e) => e.name.endsWith(".onnx"));
      const size = await fs.stat(path.join(modelDir, onnx.name));
      const sizeStr = `${(size.size / 1024 / 1024).toFixed(1)}MB`;

      console.log(
        `${entry.name.padEnd(20)} ${config.model_type.padEnd(12)} ${config.language.padEnd(8)} ${config.voice.padEnd(12)} ${sizeStr}`
      );
    } catch (error) {
      console.log(`${entry.name.padEnd(20)} <无法读取配置>`);
    }
  }
}

// 删除模型
async function deleteModel(modelKey) {
  const modelDir = path.join(MODEL_DIR, modelKey);

  try {
    await fs.access(modelDir);
    await fs.rm(modelDir, { recursive: true, force: true });
    console.log(`✓ 模型已删除: ${modelKey}`);
  } catch (error) {
    if (error.code === "ENOENT") {
      console.log(`❌ 模型不存在: ${modelKey}`);
    } else {
      console.error(`❌ 删除模型失败: ${error.message}`);
    }
    process.exit(1);
  }
}

// 主函数
async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0 || args[0] === "--help" || args[0] === "-h") {
    showHelp();
  } else if (args[0] === "--list") {
    await listDownloadedModels();
  } else if (args[0] === "--delete" && args[1]) {
    await deleteModel(args[1]);
  } else if (args[0] === "--models") {
    showModelList();
  } else if (args[0]) {
    const modelKey = args[0];
    await downloadModel(modelKey);
  } else {
    showHelp();
  }
}

// 运行主函数
main().catch((error) => {
  console.error(`❌ 脚本执行失败: ${error.message}`);
  process.exit(1);
});
