// download-onnxruntime.mjs
//
// 下载微软官方 onnxruntime Windows x64 动态库，用于解决旧 CPU（无 AVX2，
// 如三代酷睿）兼容问题：
//   - pyke 预编译的 onnxruntime 按 x86-64-v3（要求 AVX2/FMA）编译，旧 CPU 上
//     启动即非法指令崩溃；
//   - 微软官方包为 SSE3 基线 + MLAS 运行时指令集 dispatch，兼容旧 CPU。
//
// 用法:
//   node scripts/download-onnxruntime.mjs
//
// 输出:
//   src-tauri/binaries/onnxruntime.dll
//
// 由开发者/CI 在构建前手动或自动调用。
// 版本说明：默认 1.27.1，因为 ort 2.0.0-rc.13 编译时默认 api-27（对应 onnxruntime
// 1.27+），更低的版本（如 1.17.x）会被 ort::init_from 以 BadVersion 拒绝。

import { createWriteStream, existsSync, mkdirSync, readdirSync, renameSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { pipeline } from 'node:stream/promises'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const projectRoot = resolve(__dirname, '..')
const outDir = join(projectRoot, 'src-tauri', 'binaries')
const outFile = join(outDir, 'onnxruntime.dll')

// 官方 onnxruntime 版本（win-x64）。必须 ≥1.27 以匹配 ort 2.0.0-rc.13 默认 api-27。
// 官方 Windows 包为 SSE3 基线 + MLAS 运行时 CPUID dispatch（AVX2 路径受保护），
// 兼容无 AVX2 的旧 CPU（如三代酷睿）。
const ORT_VERSION = process.env.ORT_VERSION || '1.27.1'
const ZIP_URL = `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-x64-${ORT_VERSION}.zip`

// 递归查找文件（官方 zip 内有顶层目录 onnxruntime-win-x64-<ver>/，
// dll 实际在 <顶层>/lib/onnxruntime.dll，故不能用固定相对路径）
function findFile(dir, filename) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) {
      const found = findFile(full, filename)
      if (found) return found
    } else if (entry === filename) {
      return full
    }
  }
  return null
}

async function download(url, dest) {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText} for ${url}`)
  await pipeline(res.body, createWriteStream(dest))
}

async function main() {
  if (existsSync(outFile)) {
    const size = (await import('node:fs')).statSync(outFile).size
    console.log(`✅ onnxruntime.dll 已存在: ${outFile} (${(size / 1024 / 1024).toFixed(1)} MB)`)
    return
  }

  mkdirSync(outDir, { recursive: true })
  const tmpZip = join(outDir, `onnxruntime-win-x64-${ORT_VERSION}.zip`)

  console.log(`⬇️  下载 ${ZIP_URL}`)
  await download(ZIP_URL, tmpZip)
  console.log('✅ 下载完成，解压 onnxruntime.dll ...')

  // 用系统 unzip（Windows 自带 tar 可解 zip）
  const { execSync } = await import('node:child_process')
  const extractDir = join(outDir, `extract-${ORT_VERSION}`)
  mkdirSync(extractDir, { recursive: true })
  try {
    execSync(`tar -xf "${tmpZip}" -C "${extractDir}"`, { stdio: 'inherit' })
  } catch {
    // 回退：用 PowerShell Expand-Archive
    execSync(
      `powershell -NoProfile -Command "Expand-Archive -Path '${tmpZip}' -DestinationPath '${extractDir}' -Force"`,
      { stdio: 'inherit' },
    )
  }

  // 递归查找解压出的 onnxruntime.dll（兼容顶层目录结构，tar / Expand-Archive 均可）
  const dll = findFile(extractDir, 'onnxruntime.dll')
  if (!dll) {
    throw new Error(`解压后未找到 onnxruntime.dll（${extractDir}），请检查包结构`)
  }
  renameSync(dll, outFile)

  // 清理
  await import('node:fs/promises').then((fs) => fs.rm(extractDir, { recursive: true, force: true }))
  await import('node:fs/promises').then((fs) => fs.rm(tmpZip, { force: true }))

  const size = (await import('node:fs')).statSync(outFile).size
  console.log(`✅ onnxruntime.dll 就绪: ${outFile} (${(size / 1024 / 1024).toFixed(1)} MB)`)
  console.log('   开发运行：请把它复制到 exe 同目录（如 src-tauri/target/debug/onnxruntime.dll）')
}

main().catch((e) => {
  console.error('❌ 下载 onnxruntime 失败:', e.message)
  process.exit(1)
})
