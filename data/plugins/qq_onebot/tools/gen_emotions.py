# -*- coding: utf-8 -*-
"""重新生成「情绪 → 立绘文件名」表（换角色/换素材后跑一次）。

用法：
    python gen_emotions.py                      # 默认扫内测版 DeepSeek 的 avatar
    python gen_emotions.py "D:/某角色/avatar"    # 指定目录

为什么要有这个脚本：插件运行在 LingChat 的受限解释器里，`os` / `math` 都被禁，
没法在运行时检查文件是否存在，所以把映射做成静态表写进 rules.py。
"""
from __future__ import annotations

import io
import os
import re
import sys

DEFAULT_DIR = r"C:\Users\15105\tools\内测LC\data\game_data\characters\DeepSeek\avatar"
RULES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "rules.py")


def main() -> int:
    d = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DIR
    if not os.path.isdir(d):
        print(f"目录不存在: {d}")
        return 1
    names = sorted(
        os.path.splitext(f)[0] for f in os.listdir(d)
        if os.path.splitext(f)[1].lower() in (".webp", ".png")
    )
    if not names:
        print(f"{d} 里没有 webp/png")
        return 1
    ext = ".webp" if any(f.endswith(".webp") for f in os.listdir(d)) else ".png"
    rows = "\n".join(f'    "{n}": "{n}{ext}",' for n in names)
    print(f"  {len(names)} 个情绪: {', '.join(names)}")

    src = io.open(RULES, encoding="utf-8").read()
    block = ("EMOTION_FILES = {\n" + rows + "\n}\n"
             f'EMOTION_FALLBACK_FILE = "{"正常" if "正常" in names else names[0]}{ext}"')
    new, n = re.subn(r"EMOTION_FILES = \{.*?\n\}\nEMOTION_FALLBACK_FILE = \"[^\"]*\"",
                     block, src, flags=re.S)
    if n == 0:
        print("  ✘ rules.py 里找不到 EMOTION_FILES 块")
        return 1
    io.open(RULES, "w", encoding="utf-8").write(new)
    print(f"  ✔ 已写回 {RULES}")
    print(f"  记得把插件配置里的 emotion_image_dir 改成: {d.replace(chr(92), '/')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
