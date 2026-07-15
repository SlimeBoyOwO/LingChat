// download_emotion_model.mjs
import { promises as fs } from 'fs';
import path from 'path';
import { createHash } from 'crypto';
import readline from 'readline';

// 使用 process.cwd() 获取命令行所在路径（当前工作目录）
const TARGET_DIR = path.join(process.cwd(), 'data', 'third_party', 'emotion_model_19emo');

// 需要下载的文件列表（文件名与远程 URL 映射）
const FILES = [
  'config.json',
  'label_mapping.json',
  'model.onnx',
  'special_tokens_map.json',
  'tokenizer.json',
  'tokenizer_config.json',
  'vocab.txt'
];

const BASE_URL = 'https://www.modelscope.cn/models/lingchat-research-studio/Emotion_model_19emo_small_onnx/resolve/master/model_int8_o2';

// 是否启用交互式确认（可通过环境变量控制）
const INTERACTIVE = process.env.INTERACTIVE !== 'false'; // 默认启用

/**
 * 计算文件的 SHA-256 哈希值
 */
async function calculateSha256(filePath) {
  try {
    const fileBuffer = await fs.readFile(filePath);
    const hash = createHash('sha256');
    hash.update(fileBuffer);
    return hash.digest('hex');
  } catch (err) {
    if (err.code === 'ENOENT') return null;
    throw err;
  }
}

/**
 * 下载单个文件并返回内容（用于 SHA 对比）
 */
async function downloadFileContent(fileName) {
  const url = `${BASE_URL}/${fileName}`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
  }
  const buffer = await response.arrayBuffer();
  return Buffer.from(buffer);
}

/**
 * 下载单个文件并保存到磁盘
 */
async function downloadAndSave(fileName, content) {
  const destPath = path.join(TARGET_DIR, fileName);
  await fs.writeFile(destPath, content);
  return fileName;
}

/**
 * 交互式确认：是否删除并重新下载
 */
function askUserConfirmation(fileName) {
  return new Promise((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    
    rl.question(
      `\n⚠️  ${fileName} 文件内容与远程版本不一致。\n` +
      `   是否删除本地文件并重新下载？(y/N) `,
      (answer) => {
        rl.close();
        resolve(answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes');
      }
    );
  });
}

/**
 * 检查目标目录下是否存在任意 .onnx 文件
 */
async function hasOnnxFile(dir) {
  try {
    const entries = await fs.readdir(dir);
    return entries.some(entry => entry.endsWith('.onnx'));
  } catch (err) {
    if (err.code === 'ENOENT') return false;
    throw err;
  }
}

/**
 * 主函数
 */
async function main() {
  console.log(`📂 工作目录: ${process.cwd()}`);
  console.log(`📁 目标目录: ${TARGET_DIR}`);
  
  // 1. 检查是否已有 .onnx 文件
  const onnxExists = await hasOnnxFile(TARGET_DIR);
  
  // 2. 确保目录存在
  await fs.mkdir(TARGET_DIR, { recursive: true });

  if (onnxExists) {
    console.log(`✅ 目录中已存在 .onnx 文件`);
    
    // 下载 config.json 用于对比
    console.log(`🔍 正在下载远程 config.json 进行版本对比...`);
    let remoteConfigContent;
    try {
      remoteConfigContent = await downloadFileContent('config.json');
    } catch (err) {
      console.error(`❌ 无法下载远程 config.json:`, err.message);
      console.log(`💡 跳过版本检查，保留现有文件。`);
      return;
    }

    // 计算本地 config.json 的 SHA
    const localConfigPath = path.join(TARGET_DIR, 'config.json');
    const localSha = await calculateSha256(localConfigPath);
    const remoteSha = createHash('sha256').update(remoteConfigContent).digest('hex');

    if (localSha === remoteSha) {
      console.log(`✅ 本地 config.json 与远程版本一致，无需更新。`);
      return;
    }

    console.log(`🔄 检测到 config.json 版本不一致:`);
    console.log(`   本地 SHA: ${localSha}`);
    console.log(`   远程 SHA: ${remoteSha}`);

    // 交互式确认
    if (INTERACTIVE && process.stdin.isTTY) {
      const shouldReDownload = await askUserConfirmation('config.json');
      if (!shouldReDownload) {
        console.log(`⏭️  已跳过重新下载，保留现有所有文件。`);
        return;
      }
      
      console.log(`🗑️  正在删除现有文件...`);
      // 删除目录下所有文件
      const entries = await fs.readdir(TARGET_DIR);
      for (const entry of entries) {
        const entryPath = path.join(TARGET_DIR, entry);
        const stat = await fs.stat(entryPath);
        if (stat.isFile()) {
          await fs.unlink(entryPath);
          console.log(`  已删除: ${entry}`);
        }
      }
      console.log(`✅ 清理完成，开始重新下载所有文件...`);
    } else {
      console.log(`⏭️  非交互模式，跳过重新下载。`);
      console.log(`💡 如需强制重新下载，请设置环境变量 INTERACTIVE=true 或手动删除文件后重试。`);
      return;
    }
  } else {
    console.log(`📁 未找到 .onnx 文件，开始下载...`);
  }

  // 3. 并行下载所有文件（如果是重新下载或首次下载）
  console.log(`📥 开始并行下载 ${FILES.length} 个文件...`);
  
  const downloadPromises = FILES.map(async (fileName) => {
    try {
      const content = await downloadFileContent(fileName);
      await downloadAndSave(fileName, content);
      console.log(`✅ 已下载: ${fileName}`);
      return { fileName, status: 'fulfilled' };
    } catch (err) {
      console.error(`❌ 下载 ${fileName} 失败:`, err.message);
      return { fileName, status: 'rejected', error: err };
    }
  });

  const results = await Promise.allSettled(
    downloadPromises.map(p => p.then(
      result => result,
      err => ({ fileName: 'unknown', status: 'rejected', error: err })
    ))
  );

  // 统计结果
  const succeeded = results.filter(r => r.status === 'fulfilled' && r.value.status === 'fulfilled').length;
  const failed = results.filter(r => r.status === 'rejected' || r.value?.status === 'rejected').length;

  if (failed === 0) {
    console.log(`🎉 所有 ${succeeded} 个文件下载完成，保存至: ${TARGET_DIR}`);
  } else {
    console.log(`⚠️ 下载完成: ${succeeded} 个成功，${failed} 个失败`);
    process.exit(1);
  }
}

main().catch(err => {
  console.error('❌ 脚本执行出错:', err);
  process.exit(1);
});