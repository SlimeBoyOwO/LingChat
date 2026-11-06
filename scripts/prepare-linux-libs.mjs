/**
 * 打包前收集 Linux 运行期动态库，供 bundle 放进安装包的 /usr/lib/LingChat/lib。
 *
 * ort 的 webgpu 后端把 Dawn 以动态库（libwebgpu_dawn.so）形式提供，无法静态链接。
 * ort 只在 target/<profile> 下放一个指向 ~/.cache/ort.pyke.io 的软链接——那是构建机
 * 路径，绝不能进安装包，所以这里解引用复制成实体文件。
 *
 * 由 tauri.linux.conf.json 的 beforeBundleCommand 调用；该配置同时把本目录映射进
 * deb / AppImage，可执行文件靠 build.rs 写入的 RPATH 从那里加载。
 * 纯 CPU 的架构（如 Linux aarch64）没有这类库，目录留空即可。
 *
 * 新增第三方运行期库时，加进 RUNTIME_LIBS 即可。
 */

import { copyFileSync, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const destDir = join(projectRoot, "src-tauri", "linux-libs");

// 需要随包分发的第三方动态库
const RUNTIME_LIBS = ["libwebgpu_dawn.so"];

// beforeBundleCommand 在 debug 构建时带 TAURI_ENV_DEBUG=true；release 优先，
// debug 作为兜底（本地只构建过 debug 时也能打包）。
const profileDirs =
  process.env.TAURI_ENV_DEBUG === "true"
    ? ["target/debug"]
    : ["target/release", "target/debug"];

if (process.platform !== "linux") {
  console.log("[linux-libs] 非 Linux 平台，跳过");
  process.exit(0);
}

// 清空重建：避免上一次不同架构构建残留的库被打进包里
rmSync(destDir, { recursive: true, force: true });
mkdirSync(destDir, { recursive: true });

let copied = 0;
for (const name of RUNTIME_LIBS) {
  const src = profileDirs
    .map((dir) => join(projectRoot, dir, name))
    .find((p) => existsSync(p));

  if (!src) {
    console.log(`[linux-libs] 未找到 ${name}（本架构为静态链接，无需分发）`);
    continue;
  }

  // copyFileSync 会解引用软链接，落盘的是实体文件
  copyFileSync(src, join(destDir, name));
  const mb = (statSync(join(destDir, name)).size / 1024 / 1024).toFixed(1);
  console.log(`[linux-libs] ✅ ${name} (${mb} MB) ← ${src}`);
  copied++;
}

console.log(`[linux-libs] 完成，共 ${copied} 个库`);
