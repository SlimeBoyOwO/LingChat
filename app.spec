# -*- mode: python ; coding: utf-8 -*-
import sys
from pathlib import Path
from PyInstaller.utils.hooks import collect_submodules

project_root = Path(__file__).resolve().parent
main_script = str(project_root / "ling_chat" / "main.py")

block_cipher = None

a = Analysis(
    [main_script],
    pathex=[str(project_root)],
    binaries=[],
    datas=[
        # 静态资源
        ("ling_chat/static", "ling_chat/static"),
        # 数据库文件
        ("data/app.db", "data"),
    ],
    hiddenimports=collect_submodules('ling_chat'),  # 确保子模块能收集
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="ling_chat",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,  # 设置为 False 关闭控制台窗口（适用于 GUI）
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name="ling_chat"
)
