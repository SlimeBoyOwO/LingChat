/**
 * tauri CLI 分发器。
 *
 * 拦截 `pnpm tauri ...` 的所有调用（package.json 的 `tauri` 脚本指向本文件）：
 * - 子命令为 `dev` 时，先执行 `pnpm format`（prettier + cargo fmt）再启动 tauri dev；
 * - 其它子命令（build / icon / android / ios ...）原样透传，不受影响。
 *
 * 用 Node 而非在 package.json 里直接 `pnpm format && tauri`，是为了只在 dev 时格式化，
 * 避免 `tauri build`/`tauri icon` 等命令也被迫跑一遍全量格式化。
 */

import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
const subcommand = args[0];

if (subcommand === "dev") {
  console.log("⚡ tauri dev 前先执行 pnpm format ...");
  const fmt = spawnSync("pnpm", ["format"], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (fmt.status !== 0) {
    process.exit(fmt.status ?? 1);
  }
}

const result = spawnSync("tauri", args, {
  stdio: "inherit",
  shell: process.platform === "win32",
});

if (result.error) {
  console.error("❌ 启动 tauri 失败:", result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
