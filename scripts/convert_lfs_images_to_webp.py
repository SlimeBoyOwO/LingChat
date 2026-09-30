#!/usr/bin/env python3
"""
将 data/ 目录下所有被 git 跟踪的非 WebP 图片转换为 WebP 格式。

用法：
    python scripts/convert_lfs_images_to_webp.py [--dry-run] [--quality 90]

执行内容：
    1. 找出 data/ 下被 git 跟踪的图片文件（png/jpg/jpeg/gif/bmp/tiff）
    2. 用 Pillow 将每张图片转为 WebP（质量 90，保留 alpha 通道）
    3. 从 git 索引中移除旧文件，添加新的 webp 文件
    4. 从磁盘上删除原始图片文件
    5. 更新 data/ 下所有文本文件（JSON、YAML）中的路径引用
    6. 通过 scripts/generate-data-manifest.js 重新生成 data_manifest.json

    目标是缩减 Git LFS 的存储占用：相比 PNG/JPEG，WebP 在质量 90 下
    通常可获得 30-70% 的体积缩减。

依赖：
    pip install Pillow

注意：
    脚本运行后，还需检查 src/ 中硬编码的图片引用（例如
    src/stores/modules/ui/ui.ts 里的 DEFAULT_AVATAR 指向 '头像.png'，
    应改为 '头像.webp'）。
"""

import os
import subprocess
import sys
from pathlib import Path

# 修正 Windows 下的 Unicode 输出问题（GBK 编码无法处理 emoji）
if sys.platform == "win32":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore

try:
    from PIL import Image
except ImportError:
    print("❌ 需要 Pillow，请安装：pip install Pillow")
    sys.exit(1)


# ─── 配置 ─────────────────────────────────────────────────────────

QUALITY = 90
IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tiff", ".tif"}

# data/ 下可能引用图片路径的文本文件（按扩展名匹配）
REFERENCE_GLOBS = ["*.json", "*.yaml", "*.yml", "*.txt"]


# ─── 辅助函数 ─────────────────────────────────────────────────────

def run(cmd, **kwargs):
    """执行命令并返回 stdout。失败时打印 stderr。"""
    result = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", **kwargs)
    if result.returncode != 0 and result.stderr:
        print(f"  ⚠ git {' '.join(cmd[1:3])} stderr：{result.stderr.strip()}")
    return result.stdout.strip()


def get_git_root():
    return Path(run(["git", "rev-parse", "--show-toplevel"]))


def get_git_tracked_images(repo_root):
    """返回 data/ 下所有非 webp 图片的路径列表（相对于仓库根目录）。"""
    stdout = run(["git", "-c", "core.quotepath=false", "ls-files", "data/"], cwd=str(repo_root))
    images = []
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        ext = Path(line).suffix.lower()
        if ext in IMAGE_EXTENSIONS:
            full = repo_root / line
            if full.exists():
                images.append(Path(line))
    return images


def convert_to_webp(image_path, quality=QUALITY):
    """
    将单张图片转换为 WebP。
    - 对 RGBA/LA/PA/P 模式保留 alpha 通道
    - 使用配置质量的有损压缩
    - method=6 压缩率最佳（速度较慢，但只跑一次无所谓）
    返回新生成的 .webp 文件路径。
    """
    webp_path = image_path.with_suffix(".webp")

    with Image.open(image_path) as img:
        # 决定输出模式
        if img.mode in ("RGBA", "LA", "PA"):
            img = img.convert("RGBA")
        elif img.mode == "P":
            if img.info.get("transparency") is not None:
                img = img.convert("RGBA")
            else:
                img = img.convert("RGB")
        else:
            img = img.convert("RGB")

        img.save(webp_path, "webp", quality=quality, method=6)

    return webp_path


def replace_in_file(file_path, old_str, new_str):
    """将文本文件中所有 old_str 替换为 new_str。
    返回替换次数。"""
    try:
        content = file_path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return 0  # 跳过二进制文件

    # 统一成正斜杠，保证匹配一致
    old_norm = old_str.replace("\\", "/")
    new_norm = new_str.replace("\\", "/")

    count = content.count(old_norm)
    if count > 0:
        new_content = content.replace(old_norm, new_norm)
        file_path.write_text(new_content, encoding="utf-8")
    return count


def find_and_replace_references(repo_root, old_rel, new_rel):
    """
    在 data/ 下所有文本文件中搜索 old_rel 的引用并替换为 new_rel。
    old_rel 与 new_rel 是相对于仓库根目录的路径
    （例如 'data/game_data/.../foo.png'）。
    """
    # 同时构造「完整路径」与「去掉 data/ 前缀的相对路径」两种形式
    # 完整路径："data/game_data/characters/.../foo.png"
    old_full = str(old_rel).replace("\\", "/")
    new_full = str(new_rel).replace("\\", "/")

    # 相对 data/ 的路径（去掉 "data/" 前缀）："game_data/characters/.../foo.png"
    old_data_rel = old_full
    new_data_rel = new_full
    if old_full.startswith("data/"):
        old_data_rel = old_full[len("data/"):]
        new_data_rel = new_full[len("data/"):]

    # 仅文件名："foo.png" / "foo.webp"
    old_name = Path(old_rel).name
    new_name = Path(new_rel).name

    total_replaced = 0
    data_dir = repo_root / "data"

    for glob_pattern in REFERENCE_GLOBS:
        for file_path in data_dir.glob(f"**/{glob_pattern}"):
            # 替换完整路径形式
            c1 = replace_in_file(file_path, old_full, new_full)
            # 替换相对 data/ 的形式
            c2 = replace_in_file(file_path, old_data_rel, new_data_rel)
            # 替换仅文件名的形式（仅当 old_name 不与前两种形式重合时，避免重复计数）
            if old_name != old_data_rel and old_name != old_full:
                c3 = replace_in_file(file_path, old_name, new_name)
            else:
                c3 = 0

            total = c1 + c2 + c3
            if total > 0:
                rel_display = file_path.relative_to(repo_root)
                print(f"  📝 已更新 {rel_display} 中的 {total} 处引用")

            total_replaced += total

    return total_replaced


# ─── 主流程 ───────────────────────────────────────────────────────

def main():
    dry_run = "--dry-run" in sys.argv
    quality = QUALITY
    for i, arg in enumerate(sys.argv):
        if arg == "--quality" and i + 1 < len(sys.argv):
            quality = int(sys.argv[i + 1])

    if dry_run:
        print("🔍 DRY RUN —— 不会修改任何文件\n")

    repo_root = get_git_root()
    os.chdir(str(repo_root))
    print(f"📂 仓库根目录：{repo_root}\n")

    # ── 步骤 1：查找图片 ─────────────────────────────────────────
    print("🔍 正在扫描 data/ 下被 git 跟踪的非 WebP 图片 ...")
    images = get_git_tracked_images(repo_root)

    if not images:
        print("✅ 未发现非 WebP 图片，无需处理。")
        return

    print(f"\n共找到 {len(images)} 张待转换的图片：\n")
    total_old_size = 0
    for img in images:
        size = (repo_root / img).stat().st_size
        total_old_size += size
        print(f"  {img}  ({size / 1024:.1f} KB)")

    print(f"\n  合计：{total_old_size / 1024:.1f} KB ({total_old_size / 1024 / 1024:.1f} MB)")
    print(f"  质量：{quality}")
    print(f"  模式：{'DRY RUN' if dry_run else '实际执行'}\n")

    if dry_run:
        print("DRY RUN 结束。去掉 --dry-run 参数即可实际执行。")
        return

    # ── 步骤 2：逐张转换 ─────────────────────────────────────────
    conversions = []  # (旧文件相对路径, 新文件相对路径)

    for rel_path in images:
        full_path = repo_root / rel_path
        print(f"🖼  正在转换：{rel_path}")

        try:
            # 转换为 webp
            webp_full = convert_to_webp(full_path, quality=quality)
            webp_rel = webp_full.relative_to(repo_root)

            old_size = full_path.stat().st_size
            new_size = webp_full.stat().st_size
            ratio = new_size / old_size * 100 if old_size > 0 else 0

            print(f"   {old_size:,} → {new_size:,} 字节  （原体积的 {ratio:.0f}%）")

            if not dry_run:
                # Git：移除旧文件，添加新文件
                run(["git", "rm", "--cached", "--", str(rel_path)])
                run(["git", "add", "--", str(webp_rel)])

                # 删除原始文件
                full_path.unlink()

            conversions.append((rel_path, webp_rel))

        except Exception as e:
            print(f"   ❌ 错误：{e}")
            import traceback
            traceback.print_exc()

    # ── 步骤 3：统计节省量 ───────────────────────────────────────
    new_total = sum((repo_root / new).stat().st_size for _, new in conversions)
    old_total = total_old_size
    saved = old_total - new_total
    print(f"\n📊 体积对比：")
    print(f"   转换前：{old_total / 1024:.1f} KB ({old_total / 1024 / 1024:.1f} MB)")
    print(f"   转换后：{new_total / 1024:.1f} KB ({new_total / 1024 / 1024:.1f} MB)")
    print(f"   已节省：{saved / 1024:.1f} KB ({saved / 1024 / 1024:.1f} MB)  ({saved / old_total * 100:.0f}%)" if old_total > 0 else "")

    # ── 步骤 4：更新引用 ────────────────────────────────────────
    print(f"\n🔗 正在更新 data/ 文件中的路径引用 ...")
    total_refs = 0
    for old_rel, new_rel in conversions:
        refs = find_and_replace_references(repo_root, old_rel, new_rel)
        total_refs += refs

    if total_refs > 0:
        print(f"\n   共更新 {total_refs} 处引用")
        # 将更新过引用的文件加入暂存区
        run(["git", "add", "data/"])
    else:
        print("   未发现外部引用（manifest 会自行处理）。")

    # ── 步骤 5：重新生成 manifest ───────────────────────────────
    manifest_script = repo_root / "scripts" / "generate-data-manifest.js"
    if manifest_script.exists():
        print(f"\n🔄 正在重新生成 data_manifest.json ...")
        result = subprocess.run(
            ["node", str(manifest_script), "--output", "data/data_manifest.json"],
            capture_output=True, text=True, encoding="utf-8",
            cwd=str(repo_root),
        )
        if result.returncode != 0:
            print(f"   ⚠ manifest 生成失败：")
            print(f"   {result.stderr}")
        else:
            print(f"   {result.stdout.strip()}")
            run(["git", "add", "data/data_manifest.json"])

    # ── 完成 ─────────────────────────────────────────────────────
    print(f"\n{'=' * 60}")
    print(f"✅ 转换完成！共 {len(conversions)} 个文件转为 WebP。")
    print(f"\n后续步骤：")
    print(f"  1. 检查：  git status")
    print(f"  2. 测试：  运行应用，确认图片能正常加载")
    print(f"  3. 核对：  src/stores/modules/ui/ui.ts 中硬编码的 '头像.png' → 改为 '.webp'")
    print(f"  4. 提交：  git commit -m 'perf: convert data images to WebP to reduce LFS storage'")


if __name__ == "__main__":
    main()
