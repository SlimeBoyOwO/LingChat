#!/usr/bin/env python3
"""
批量将图片转换为 WebP 格式（通用转换工具）。

用法：
    python scripts/convert_to_webp.py -i <文件或文件夹> [-o <输出目录>] [-q <质量>] [-m <effort>] [-j <并发数>]

参数：
    -i, --input    输入文件或文件夹。传入文件夹时会递归遍历其中所有图片。
    -o, --output   输出目录。默认为输入路径下的 webp/ 文件夹。
    -q, --quality  有损压缩质量，1-100，默认 85。数值越高画质越好、体积越大。
    -m, --method   编码 effort，0-6，默认 1。越大压缩率越高，但编码越慢。
    -j, --jobs     并发进程数，默认为 CPU 核心数。设为 1 则串行执行。

支持的处理：
    - 输入格式：png / jpg / jpeg / jfif / gif / bmp / tif / tiff / webp / ico。
      已经是 webp 的文件也会按指定质量重新编码。
    - 透明通道：RGBA、灰度 + alpha，以及带 tRNS 透明色的调色板 PNG 都会被
      正确导出为带 alpha 的 WebP，不会出现黑底。
    - 动画：GIF、APNG、动态 WebP 会逐帧导出为动态 WebP，并保留每帧时长与
      循环次数。
    - 色彩与方向：透传 ICC 色彩配置；带 EXIF 方向的照片会先旋正再编码，
      避免输出后方向错误。
    - 遍历文件夹时保留原有子目录结构；输出目录若位于输入目录内会被自动
      排除，因此可以重复执行而不会反复转换自己产出的文件。

依赖：
    pip install Pillow

运行环境：
    Python 3.9+
"""

import argparse
import os
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

try:
    from PIL import Image, ImageOps
except ImportError:
    print("需要 Pillow，请先安装：pip install Pillow", file=sys.stderr)
    sys.exit(1)


# 支持转换的输入格式（均为 Pillow 内置支持）
IMAGE_EXTENSIONS = {
    ".png",
    ".jpg",
    ".jpeg",
    ".jfif",
    ".gif",
    ".bmp",
    ".tif",
    ".tiff",
    ".webp",
    ".ico",
}

DEFAULT_QUALITY = 85
# 动图未标注某帧时长时的兜底值（毫秒）
DEFAULT_FRAME_DURATION = 100
# 编码 effort（Pillow 的 method），0-6。越大压缩率越高但越慢，默认取 1 优先速度
DEFAULT_METHOD = 1
# 默认并发进程数；编码是纯 CPU 计算，按核心数铺满
DEFAULT_JOBS = os.cpu_count() or 1


# ─── 参数解析 ─────────────────────────────────────────────────────

def positive_int(value):
    """argparse 用的正整数校验。"""
    try:
        number = int(value)
    except ValueError:
        raise argparse.ArgumentTypeError(f"需要整数，收到 {value!r}")
    if number < 1:
        raise argparse.ArgumentTypeError(f"必须 >= 1，收到 {number}")
    return number


def parse_args(argv=None):
    parser = argparse.ArgumentParser(
        description="批量将图片转换为 WebP 格式。",
    )
    parser.add_argument(
        "-i", "--input",
        required=True,
        help="输入文件或文件夹（文件夹会递归遍历其中所有图片）",
    )
    parser.add_argument(
        "-o", "--output",
        help="输出目录，默认为输入路径下的 webp/ 文件夹",
    )
    parser.add_argument(
        "-q", "--quality",
        type=int,
        default=DEFAULT_QUALITY,
        help=f"有损压缩质量 1-100，默认 {DEFAULT_QUALITY}",
    )
    parser.add_argument(
        "-m", "--method",
        type=int,
        choices=range(7),
        default=DEFAULT_METHOD,
        help=f"编码 effort 0-6，越大压缩率越高但越慢，默认 {DEFAULT_METHOD}",
    )
    parser.add_argument(
        "-j", "--jobs",
        type=positive_int,
        default=DEFAULT_JOBS,
        help=f"并发进程数，默认 {DEFAULT_JOBS}（CPU 核心数），1 表示串行",
    )
    args = parser.parse_args(argv)

    if not 1 <= args.quality <= 100:
        parser.error(f"--quality 必须在 1-100 之间，收到 {args.quality}")

    return args


# ─── 路径处理 ─────────────────────────────────────────────────────

def collect_images(input_path, out_dir):
    """收集所有待转换的图片文件。

    文件夹会递归遍历；隐藏文件/目录（如 .git）以及位于输出目录内的文件会被跳过，
    这样把输出目录放在输入目录里面也能安全地反复执行。
    """
    if input_path.is_file():
        return [input_path]

    out_resolved = out_dir.resolve()
    images = []
    for path in sorted(input_path.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in IMAGE_EXTENSIONS:
            continue
        if any(part.startswith(".") for part in path.relative_to(input_path).parts):
            continue
        if path.resolve().is_relative_to(out_resolved):
            continue
        images.append(path)
    return images


def build_dest(src, input_path, out_dir):
    """计算输出路径。遍历文件夹时保留原有的子目录结构。"""
    if input_path.is_file():
        return out_dir / f"{src.stem}.webp"
    return out_dir / src.relative_to(input_path).with_suffix(".webp")


def display_name(src, input_path):
    """输出用的显示名：单个文件给文件名，文件夹给相对路径。"""
    if input_path.is_file():
        return src.name
    return str(src.relative_to(input_path))


# ─── 转换 ─────────────────────────────────────────────────────────

def save_static(img, dst, quality, method):
    """转换单帧图片：保留透明通道，按 EXIF 方向旋正，透传 ICC 配置。"""
    img = ImageOps.exif_transpose(img)

    if img.mode in ("RGBA", "LA", "PA") or (img.mode == "P" and "transparency" in img.info):
        # 带 alpha 的图，以及带 tRNS 透明色的调色板 PNG
        out = img.convert("RGBA")
    else:
        out = img.convert("RGB")

    out.save(
        dst,
        "WEBP",
        quality=quality,
        method=method,
        icc_profile=img.info.get("icc_profile"),
    )


def save_animated(img, dst, quality, method):
    """把动图（GIF / APNG / 动态 WebP）逐帧导出为动态 WebP。

    Pillow 在 seek 时会自行处理 GIF 的帧间合成（局部帧、disposal 方式），
    所以逐帧 convert 得到的就是合成后的画面。
    """
    frames = []
    durations = []
    for index in range(img.n_frames):
        img.seek(index)
        frames.append(img.convert("RGBA"))
        durations.append(img.info.get("duration", DEFAULT_FRAME_DURATION))

    frames[0].save(
        dst,
        "WEBP",
        save_all=True,
        append_images=frames[1:],
        duration=durations,
        loop=img.info.get("loop", 0),
        quality=quality,
        method=method,
        icc_profile=img.info.get("icc_profile"),
    )


def convert(src, dst, quality, method):
    """把单个图片文件转换为 WebP，返回 (原字节数, 新字节数)。"""
    dst.parent.mkdir(parents=True, exist_ok=True)
    before = src.stat().st_size

    with Image.open(src) as img:
        if getattr(img, "is_animated", False) and getattr(img, "n_frames", 1) > 1:
            save_animated(img, dst, quality, method)
        else:
            save_static(img, dst, quality, method)

    return before, dst.stat().st_size


# ─── 并发调度 ─────────────────────────────────────────────────────

def convert_task(task):
    """在子进程中执行单个转换任务。

    异常在这里就地转成字符串：异常对象本身不一定能跨进程序列化，
    带到主进程统一汇报更稳妥。
    """
    label, src, dst, quality, method = task
    try:
        before, after = convert(src, dst, quality, method)
        return label, before, after, None
    except Exception as exc:
        return label, 0, 0, f"{type(exc).__name__}: {exc}"


def process_tasks(tasks, jobs):
    """按并发度执行任务，按提交顺序产出 (label, before, after, error)。"""
    if jobs <= 1:
        for task in tasks:
            yield convert_task(task)
        return

    with ProcessPoolExecutor(max_workers=jobs) as pool:
        yield from pool.map(convert_task, tasks, chunksize=1)


# ─── 主流程 ───────────────────────────────────────────────────────

def main(argv=None):
    args = parse_args(argv)

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"错误：输入路径不存在：{input_path}", file=sys.stderr)
        return 2

    base_dir = input_path if input_path.is_dir() else input_path.parent
    out_dir = Path(args.output) if args.output else base_dir / "webp"

    images = collect_images(input_path, out_dir)
    if not images:
        print(f"未在 {input_path} 中找到可转换的图片。")
        return 0

    out_dir.mkdir(parents=True, exist_ok=True)
    print(f"输入：  {input_path}")
    print(f"输出：  {out_dir}")
    print(f"质量：  {args.quality}")
    print(f"effort：{args.method}")
    print(f"并发：  {args.jobs} 个进程")

    # 先把任务列表建好：跳过与输出同路径的文件，其余统一交给进程池。
    # 路径先 resolve 成绝对路径，避免子进程的工作目录与主进程不一致。
    tasks = []
    skipped = 0
    for src in images:
        dst = build_dest(src, input_path, out_dir)
        label = display_name(src, input_path)
        src_abs, dst_abs = src.resolve(), dst.resolve()

        if src_abs == dst_abs:
            print(f"跳过（输出路径与输入相同）：{label}")
            skipped += 1
            continue

        tasks.append((label, src_abs, dst_abs, args.quality, args.method))

    print(f"待转换：{len(tasks)} 个文件\n")

    converted = 0
    failures = []
    before_total = 0
    after_total = 0

    for label, before, after, error in process_tasks(tasks, args.jobs):
        if error is not None:
            print(f"失败：{label} -> {error}", file=sys.stderr)
            failures.append((label, error))
            continue

        before_total += before
        after_total += after
        converted += 1

        ratio = f"{after / before * 100:.0f}%" if before else "?"
        print(f"  {label}  {before / 1024:.1f} KB -> {after / 1024:.1f} KB ({ratio})")

    print()
    print("=" * 60)
    print(f"完成：{converted} 个已转换，{skipped} 个跳过，{len(failures)} 个失败")
    if converted and before_total:
        saved = before_total - after_total
        print(
            f"体积：{before_total / 1024 / 1024:.2f} MB -> {after_total / 1024 / 1024:.2f} MB"
            f"（节省 {saved / 1024 / 1024:.2f} MB，{saved / before_total * 100:.1f}%）"
        )

    if failures:
        print("\n失败列表：")
        for label, exc in failures:
            print(f"  {label}: {exc}")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
