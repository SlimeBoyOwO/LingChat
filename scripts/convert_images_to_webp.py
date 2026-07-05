#!/usr/bin/env python3
"""
Convert all git-tracked non-WebP image files under data/ to WebP format.

Usage:
    python scripts/convert_images_to_webp.py [--dry-run] [--quality 90]

What it does:
    1. Finds git-tracked image files (png/jpg/jpeg/gif/bmp/tiff) under data/
    2. Converts each to WebP using Pillow (quality 90, preserves alpha channel)
    3. Removes old files from git index, adds new webp files
    4. Deletes the original image files from disk
    5. Updates all path references in text files under data/ (JSON, YAML)
    6. Re-generates data_manifest.json via scripts/generate-data-manifest.js

    The goal is to reduce Git LFS storage by using WebP, which typically achieves
    30-70% smaller file sizes compared to PNG/JPEG at quality 90.

Requirements:
    pip install Pillow

Note:
    After running this script, you should also check src/ for hardcoded image
    references (e.g. src/stores/modules/ui/ui.ts has a DEFAULT_AVATAR pointing
    to '头像.png' which should become '头像.webp').
"""

import os
import subprocess
import sys
from pathlib import Path

# Fix Unicode output on Windows (GBK codec can't handle emoji)
if sys.platform == "win32":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore

try:
    from PIL import Image
except ImportError:
    print("❌ Pillow is required. Install with: pip install Pillow")
    sys.exit(1)


# ─── Configuration ────────────────────────────────────────────────

QUALITY = 90
IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tiff", ".tif"}

# Text files under data/ that may reference image paths (searched by extension)
REFERENCE_GLOBS = ["*.json", "*.yaml", "*.yml", "*.txt"]


# ─── Helpers ──────────────────────────────────────────────────────

def run(cmd, **kwargs):
    """Run a command, return stdout. Prints stderr on failure."""
    result = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", **kwargs)
    if result.returncode != 0 and result.stderr:
        print(f"  ⚠ git {' '.join(cmd[1:3])} stderr: {result.stderr.strip()}")
    return result.stdout.strip()


def get_git_root():
    return Path(run(["git", "rev-parse", "--show-toplevel"]))


def get_git_tracked_images(repo_root):
    """Return list of Paths (relative to repo root) of non-webp images under data/."""
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
    Convert an image to WebP.
    - Preserves alpha channel for RGBA/LA/PA/P images
    - Uses lossy compression at configured quality
    - method=6 for best compression (slower but run-once is fine)
    Returns the Path to the new .webp file.
    """
    webp_path = image_path.with_suffix(".webp")

    with Image.open(image_path) as img:
        # Determine output mode
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
    """Replace all occurrences of old_str with new_str in a text file.
    Returns the number of replacements made."""
    try:
        content = file_path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return 0  # skip binary files

    # Normalize to forward slashes for consistent matching
    old_norm = old_str.replace("\\", "/")
    new_norm = new_str.replace("\\", "/")

    count = content.count(old_norm)
    if count > 0:
        new_content = content.replace(old_norm, new_norm)
        file_path.write_text(new_content, encoding="utf-8")
    return count


def find_and_replace_references(repo_root, old_rel, new_rel):
    """
    Search all text files under data/ for references to old_rel and replace with new_rel.
    old_rel and new_rel are paths relative to repo root (e.g. 'data/game_data/.../foo.png').
    """
    # Build both full-path and data-relative forms
    # Full path: "data/game_data/characters/.../foo.png"
    old_full = str(old_rel).replace("\\", "/")
    new_full = str(new_rel).replace("\\", "/")

    # Data-relative path (without "data/" prefix): "game_data/characters/.../foo.png"
    old_data_rel = old_full
    new_data_rel = new_full
    if old_full.startswith("data/"):
        old_data_rel = old_full[len("data/"):]
        new_data_rel = new_full[len("data/"):]

    # Filename only: "foo.png" / "foo.webp"
    old_name = Path(old_rel).name
    new_name = Path(new_rel).name

    total_replaced = 0
    data_dir = repo_root / "data"

    for glob_pattern in REFERENCE_GLOBS:
        for file_path in data_dir.glob(f"**/{glob_pattern}"):
            # Replace full path form
            c1 = replace_in_file(file_path, old_full, new_full)
            # Replace data-relative form
            c2 = replace_in_file(file_path, old_data_rel, new_data_rel)
            # Replace filename-only form (only if old_name != old_data_rel to avoid double-count)
            if old_name != old_data_rel and old_name != old_full:
                c3 = replace_in_file(file_path, old_name, new_name)
            else:
                c3 = 0

            total = c1 + c2 + c3
            if total > 0:
                rel_display = file_path.relative_to(repo_root)
                print(f"  📝 Updated {total} ref(s) in {rel_display}")

            total_replaced += total

    return total_replaced


# ─── Main ─────────────────────────────────────────────────────────

def main():
    dry_run = "--dry-run" in sys.argv
    quality = QUALITY
    for i, arg in enumerate(sys.argv):
        if arg == "--quality" and i + 1 < len(sys.argv):
            quality = int(sys.argv[i + 1])

    if dry_run:
        print("🔍 DRY RUN — no files will be modified\n")

    repo_root = get_git_root()
    os.chdir(str(repo_root))
    print(f"📂 Repository root: {repo_root}\n")

    # ── Step 1: Find images ──────────────────────────────────────
    print("🔍 Scanning git-tracked non-WebP images under data/ ...")
    images = get_git_tracked_images(repo_root)

    if not images:
        print("✅ No non-WebP images found. Nothing to do.")
        return

    print(f"\nFound {len(images)} image(s) to convert:\n")
    total_old_size = 0
    for img in images:
        size = (repo_root / img).stat().st_size
        total_old_size += size
        print(f"  {img}  ({size / 1024:.1f} KB)")

    print(f"\n  Total: {total_old_size / 1024:.1f} KB ({total_old_size / 1024 / 1024:.1f} MB)")
    print(f"  Quality: {quality}")
    print(f"  Mode: {'DRY RUN' if dry_run else 'LIVE'}\n")

    if dry_run:
        print("Dry run complete. Remove --dry-run to execute.")
        return

    # ── Step 2: Convert each image ───────────────────────────────
    conversions = []  # (old_rel_path, new_rel_path)

    for rel_path in images:
        full_path = repo_root / rel_path
        print(f"🖼  Converting: {rel_path}")

        try:
            # Convert to webp
            webp_full = convert_to_webp(full_path, quality=quality)
            webp_rel = webp_full.relative_to(repo_root)

            old_size = full_path.stat().st_size
            new_size = webp_full.stat().st_size
            ratio = new_size / old_size * 100 if old_size > 0 else 0

            print(f"   {old_size:,} → {new_size:,} bytes  ({ratio:.0f}% of original)")

            if not dry_run:
                # Git: remove old, add new
                run(["git", "rm", "--cached", "--", str(rel_path)])
                run(["git", "add", "--", str(webp_rel)])

                # Delete original file
                full_path.unlink()

            conversions.append((rel_path, webp_rel))

        except Exception as e:
            print(f"   ❌ ERROR: {e}")
            import traceback
            traceback.print_exc()

    # ── Step 3: Calculate savings ────────────────────────────────
    new_total = sum((repo_root / new).stat().st_size for _, new in conversions)
    old_total = total_old_size
    saved = old_total - new_total
    print(f"\n📊 Size comparison:")
    print(f"   Before: {old_total / 1024:.1f} KB ({old_total / 1024 / 1024:.1f} MB)")
    print(f"   After:  {new_total / 1024:.1f} KB ({new_total / 1024 / 1024:.1f} MB)")
    print(f"   Saved:  {saved / 1024:.1f} KB ({saved / 1024 / 1024:.1f} MB)  ({saved / old_total * 100:.0f}%)" if old_total > 0 else "")

    # ── Step 4: Update references ─────────────────────────────────
    print(f"\n🔗 Updating path references in data/ files ...")
    total_refs = 0
    for old_rel, new_rel in conversions:
        refs = find_and_replace_references(repo_root, old_rel, new_rel)
        total_refs += refs

    if total_refs > 0:
        print(f"\n   Total references updated: {total_refs}")
        # Stage the updated reference files
        run(["git", "add", "data/"])
    else:
        print("   No external references found (manifest handles its own).")

    # ── Step 5: Re-generate manifest ─────────────────────────────
    manifest_script = repo_root / "scripts" / "generate-data-manifest.js"
    if manifest_script.exists():
        print(f"\n🔄 Re-generating data_manifest.json ...")
        result = subprocess.run(
            ["node", str(manifest_script), "--output", "data/data_manifest.json"],
            capture_output=True, text=True, encoding="utf-8",
            cwd=str(repo_root),
        )
        if result.returncode != 0:
            print(f"   ⚠ Manifest generation failed:")
            print(f"   {result.stderr}")
        else:
            print(f"   {result.stdout.strip()}")
            run(["git", "add", "data/data_manifest.json"])

    # ── Done ──────────────────────────────────────────────────────
    print(f"\n{'=' * 60}")
    print(f"✅ Conversion complete! {len(conversions)} file(s) converted to WebP.")
    print(f"\nNext steps:")
    print(f"  1. Review:  git status")
    print(f"  2. Test:    Run the app and verify images load correctly")
    print(f"  3. Check:   src/stores/modules/ui/ui.ts has hardcoded '头像.png' → update to '.webp'")
    print(f"  4. Commit:  git commit -m 'perf: convert data images to WebP to reduce LFS storage'")


if __name__ == "__main__":
    main()
