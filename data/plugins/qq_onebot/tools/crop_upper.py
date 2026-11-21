# -*- coding: utf-8 -*-
"""把角色 avatar 全身立绘裁成「上半身特写」，供 QQ 回复配图。

和第一版的区别：不再按整张图的比例硬切，而是先算出**角色实际内容的包围盒**
（透明通道），再从内容顶部往下取一段，并按这一段的内容宽度左右收紧 ——
所以脸部会更大，左右白边也不会有。

用法：
    python crop_upper.py                                  # 默认内测版全部角色
    python crop_upper.py "D:/角色/avatar"                  # 只处理某个角色的 avatar
    python crop_upper.py --ratio 0.32 --pad-x 0.06        # 更特写 / 更窄边

参数含义：
    --ratio  从内容顶部往下取多少（占内容高度的比例）。0.36 ≈ 头顶到胸口
    --pad-x  左右各留多少边（占该段内容宽度的比例）
    --width  输出宽度（默认 720）
"""
from __future__ import annotations

import argparse
import os

try:
    from PIL import Image
except ImportError:
    print("需要 Pillow：pip install pillow")
    raise SystemExit(1)

BASE = r"C:\Users\15105\tools\内测LC\data\game_data\characters"
SKIP = {"头像"}


def content_bbox(im):
    """透明图用 alpha 求内容范围；不透明图按白色背景求。"""
    if im.mode in ("RGBA", "LA"):
        alpha = im.split()[-1]
        return alpha.point(lambda v: 255 if v > 8 else 0).getbbox()
    rgb = im.convert("RGB")
    # 白底：把接近白的像素当背景
    gray = rgb.convert("L").point(lambda v: 0 if v > 245 else 255)
    return gray.getbbox()


def crop_one(path: str, out_path: str, ratio: float, pad_x: float,
             width: int, quality: int) -> tuple[int, int, int, int]:
    im = Image.open(path).convert("RGBA")
    W, H = im.size
    box = content_bbox(im) or (0, 0, W, H)
    x0, y0, x1, y1 = box
    content_w, content_h = x1 - x0, y1 - y0

    # 1) 垂直：从内容顶部往下取 ratio 段
    band_bottom = min(H, y0 + max(1, int(content_h * ratio)))
    band = im.crop((0, y0, W, band_bottom))

    # 2) 水平：按这一段自己的内容宽度收紧并居中
    b2 = content_bbox(band) or (x0, 0, x1, band.height)
    bx0, _, bx1, _ = b2
    band_w = bx1 - bx0
    pad = int(band_w * pad_x)
    left = max(0, bx0 - pad)
    right = min(W, bx1 + pad)

    out = im.crop((left, y0, right, band_bottom))
    out = out.resize((width, int(out.height * width / out.width)), Image.LANCZOS)

    # 透明底压到白色（QQ 里有些客户端不认透明）
    bg = Image.new("RGB", out.size, (255, 255, 255))
    bg.paste(out, mask=out.split()[-1])
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    bg.save(out_path, "WEBP", quality=quality, method=4)
    return (W, H, out.width, out.height)


def do_dir(src: str, ratio: float, pad_x: float, width: int, quality: int) -> int:
    src = src.rstrip("\\/")
    if not os.path.isdir(src):
        print(f"目录不存在: {src}")
        return 0
    dst = src + "_upper"
    files = sorted(f for f in os.listdir(src)
                   if f.lower().endswith((".webp", ".png")) and os.path.splitext(f)[0] not in SKIP)
    n = 0
    for name in files:
        out = os.path.join(dst, os.path.splitext(name)[0] + ".webp")
        w0, h0, w1, h1 = crop_one(os.path.join(src, name), out, ratio, pad_x, width, quality)
        print(f"  {name:<10} {w0}x{h0} → {w1}x{h1}  {os.path.getsize(out)//1024} KB")
        n += 1
    print(f"  {n} 张 → {dst}")
    return n


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("src", nargs="?", default="")
    ap.add_argument("--ratio", type=float, default=0.36)
    ap.add_argument("--pad-x", type=float, default=0.06)
    ap.add_argument("--width", type=int, default=720)
    ap.add_argument("--quality", type=int, default=88)
    args = ap.parse_args()

    if args.src:
        do_dir(args.src, args.ratio, args.pad_x, args.width, args.quality)
        return 0

    total = 0
    for name in sorted(os.listdir(BASE)):
        av = os.path.join(BASE, name, "avatar")
        if os.path.isdir(av) and any(f.lower().endswith((".webp", ".png")) for f in os.listdir(av)):
            print(f"\n[{name}]")
            total += do_dir(av, args.ratio, args.pad_x, args.width, args.quality)
    print(f"\n  合计 {total} 张")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
