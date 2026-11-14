# -*- coding: utf-8 -*-
"""生成「角色名 → 立绘目录」映射（写进 rules.py 的 CHARACTER_DIRS）。

为什么要同时收两种名字：LC 的角色目录名和显示名经常不一致，例如
    目录 诺一钦灵  →  ai_name 钦灵
    目录 小瞬      →  ai_name 瞬
插件从 line_shown 拿到的是显示名，只按目录名匹配会 miss，退回默认目录
（表现就是"换了角色还是原来的立绘"）。

用法：
    python gen_characters.py
"""
from __future__ import annotations

import io
import os
import re
import sys

BASE = r"C:\Users\15105\tools\内测LC\data\game_data\characters"
RULES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "rules.py")


def read_ai_name(folder: str) -> str:
    path = os.path.join(folder, "settings.yml")
    if not os.path.isfile(path):
        return ""
    for line in io.open(path, encoding="utf-8", errors="replace"):
        m = re.match(r"\s*ai_name\s*:\s*(.+)", line)
        if m:
            return m.group(1).strip().strip('"').strip("'")
    return ""


def main() -> int:
    if not os.path.isdir(BASE):
        print(f"角色目录不存在: {BASE}")
        return 1

    mapping: dict[str, str] = {}
    for name in sorted(os.listdir(BASE)):
        up = os.path.join(BASE, name, "avatar_upper")
        if not (os.path.isdir(up) and any(f.endswith(".webp") for f in os.listdir(up))):
            continue
        path = up.replace("\\", "/")
        mapping[name] = path                       # 文件夹名
        ai = read_ai_name(os.path.join(BASE, name))
        if ai and ai != name and ai not in mapping:
            mapping[ai] = path                     # 显示名（ai_name）

    rows = "\n".join(f'    "{k}": "{v}",' for k, v in mapping.items())
    block = f"CHARACTER_DIRS = {{\n{rows}\n}}"

    src = io.open(RULES, encoding="utf-8").read()
    new, n = re.subn(r"CHARACTER_DIRS = \{.*?\n\}", block, src, flags=re.S)
    if n == 0:
        print("  ✘ rules.py 里找不到 CHARACTER_DIRS 块")
        return 1
    io.open(RULES, "w", encoding="utf-8").write(new)

    print(f"  {len(mapping)} 个键（文件夹名 + 显示名）:")
    for k, v in mapping.items():
        print(f"    {k:<22} → {v.split('/characters/')[-1]}")
    print(f"\n  ✔ 已写回 {RULES}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
