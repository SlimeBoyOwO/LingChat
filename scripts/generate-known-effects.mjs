/**
 * 从粒子注册表生成 Rust 侧的特效清单。
 *
 * 特效完全由前端渲染，Rust 不参与。保留这份清单只是为了让剧本编辑器
 * 能给作者报一句「这不是内置特效」。手抄一份的毛病在于抄漏了不报错 ——
 * 症状仅仅是那条警告不再触发，很难查。所以改成从注册表生成。
 *
 * 用法:
 *   node scripts/generate-known-effects.mjs           生成
 *   node scripts/generate-known-effects.mjs --check   只校验产物是否最新（不写文件）
 *
 * 唯一真相: src/components/game/standard/particles/index.ts 的 ALL_EFFECTS
 * 产物:     src-tauri/src/ai_service/game_system/script_engine/events/known_effects.rs
 */

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, "..");

const REGISTRY = join(projectRoot, "src/components/game/standard/particles/index.ts");
const OUTPUT = join(
  projectRoot,
  "src-tauri/src/ai_service/game_system/script_engine/events/known_effects.rs",
);

const checkOnly = process.argv.includes("--check");

/**
 * 把注册表编成 JS 再 import —— 比正则解析 TS 可靠得多。
 * 产物落在 node_modules 里而不是系统临时目录：注册表以后若引入别的模块，
 * 裸模块名要能在项目里解析到。
 */
async function loadRegistryKeys() {
  const tsc = join(projectRoot, "node_modules/typescript/bin/tsc");
  const outDir = join(projectRoot, "node_modules/.cache/known-effects");
  mkdirSync(outDir, { recursive: true });

  // 只给入口文件的话，tsc 会把被 import 的那些文件静默漏掉（5.6 的行为），
  // 编出来的 index.js 于是引用一堆不存在的模块。整个目录都当输入才齐
  const inputs = [];
  const collect = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) collect(path);
      else if (path.endsWith(".ts")) inputs.push(path);
    }
  };
  collect(dirname(REGISTRY));
  if (!inputs.includes(REGISTRY)) inputs.push(REGISTRY);

  execFileSync(
    process.execPath,
    [
      tsc,
      ...inputs,
      "--outDir",
      outDir,
      "--rootDir",
      dirname(REGISTRY),
      "--target",
      "es2020",
      "--module",
      "esnext",
      "--moduleResolution",
      "bundler",
      "--skipLibCheck",
    ],
    { stdio: "inherit" },
  );

  // 浏览器/Node 的 ESM 都要求相对导入带扩展名，tsc 不会补。
  // 产物跟着源码的目录结构走，所以这里也得递归
  const patch = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        patch(path);
        continue;
      }
      if (!path.endsWith(".js")) continue;
      writeFileSync(
        path,
        readFileSync(path, "utf-8").replace(
          /(from\s+")(\.[^"]*)(")/g,
          (_m, head, spec, tail) => `${head}${spec.endsWith(".js") ? spec : `${spec}.js`}${tail}`,
        ),
      );
    }
  };
  patch(outDir);

  const mod = await import(pathToFileURL(join(outDir, "index.js")).href);
  const keys = (mod.ALL_EFFECTS ?? []).map((effect) => effect.key);
  if (keys.length === 0) {
    throw new Error("注册表里没读到任何特效，ALL_EFFECTS 是不是改名了？");
  }
  return keys;
}

function render(keys) {
  const body = keys.map((key) => `    "${key.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}",`);
  return `//! 内置背景特效清单 —— 本文件由脚本生成，请勿手改。
//!
//! 生成命令: pnpm gen:effects
//! 唯一真相是前端注册表 src/components/game/standard/particles/index.ts。
//!
//! 特效由前端渲染，Rust 不参与，这份清单只用来让剧本编辑器给作者报
//! 「这不是内置特效」。正因为如此它必须是派生品 —— 手抄一份的话，抄漏的
//! 症状只是警告不再触发，不报错，很难查。
//!
//! 前端把特效分成氛围与天气两层，这里刻意拍平：剧本只写一个特效值，
//! 落在哪一层由前端决定。

/// 全部内置特效的 key，顺序与注册表一致。
///
/// 大小写敏感：前端按 === 比较，starfield 和 Starfield 都不渲染。
/// 不在这个列表里的值（含约定的 None）会清空当前特效。
pub const KNOWN_EFFECTS: &[&str] = &[
${body.join("\n")}
];
`;
}

const keys = await loadRegistryKeys();
const next = render(keys);

if (checkOnly) {
  let current = "";
  try {
    current = readFileSync(OUTPUT, "utf-8");
  } catch {
    console.error("❌ 产物不存在，请先执行 pnpm gen:effects");
    process.exit(1);
  }
  if (current !== next) {
    console.error("❌ known_effects.rs 与粒子注册表不一致，请执行 pnpm gen:effects");
    process.exit(1);
  }
  console.log(`✅ known_effects.rs 与注册表一致（${keys.length} 个特效）`);
} else {
  writeFileSync(OUTPUT, next);
  console.log(`✅ 已生成 known_effects.rs（${keys.length} 个特效）: ${keys.join(", ")}`);
}
