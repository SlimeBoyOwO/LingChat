import os
import sys
import zipfile
import tarfile
import hashlib
import json
import tempfile
import shutil
import subprocess
import multiprocessing
import requests
from pathlib import Path
from datetime import datetime
from concurrent.futures import ThreadPoolExecutor, as_completed

UPDATES_DIR = Path(__file__).parent / "updates"
DOWNLOADS_DIR = UPDATES_DIR / "downloads"
UPDATES_DIR.mkdir(parents=True, exist_ok=True)
DOWNLOADS_DIR.mkdir(parents=True, exist_ok=True)

def log(message):
    print(f"[{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] {message}")

def log_error(message):
    print(f"[{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] ERROR: {message}", file=sys.stderr)

def get_cpu_threads():
    """获取用于压缩的线程数"""
    cpu_count = multiprocessing.cpu_count()
    return max(1, cpu_count)

def is_symlink_or_junction(path):
    try:
        return os.path.islink(path) if hasattr(os, 'readlink') else False
    except Exception:
        return False

def safe_walk(base_path):
    visited = set()
    for root, dirs, files in os.walk(base_path, followlinks=False):
        real = os.path.realpath(root)
        if real in visited:
            dirs[:] = []
            continue
        visited.add(real)
        if not hasattr(os, 'readlink'):
            dirs[:] = [d for d in dirs if not is_symlink_or_junction(Path(root) / d)]
        yield root, dirs, files

def sha256_file(path):
    h = hashlib.sha256()
    try:
        with open(path, "rb") as f:
            for chunk in iter(lambda: f.read(65536), b""):
                h.update(chunk)
    except Exception:
        return "0" * 64
    return h.hexdigest()

def collect_files(base):
    files = {}
    total_size = 0
    file_list = []
    
    for root, _, filenames in safe_walk(base):
        for fn in filenames:
            full = Path(root) / fn
            try:
                if full.is_symlink():
                    continue
                rel = full.relative_to(base).as_posix()
                size = full.stat().st_size
                file_list.append((rel, full, size))
                total_size += size
            except Exception:
                continue
    
    if not file_list:
        return files
    
    def calc_hash(item):
        rel, full, _ = item
        return rel, sha256_file(full)
    
    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as ex:
        futures = {ex.submit(calc_hash, item): item for item in file_list}
        for fut in as_completed(futures):
            rel, h = fut.result()
            files[rel] = h
    
    log(f"Scanned {len(files)} files ({total_size/1024/1024:.1f} MB)")
    return files

def extract_archive(archive_path):
    tmp = Path(tempfile.mkdtemp(prefix="base_"))
    archive_path = Path(archive_path)
    
    if archive_path.suffix == '.zip':
        with zipfile.ZipFile(archive_path, "r") as zf:
            for info in zf.infolist():
                try:
                    zf.extract(info, tmp)
                except Exception:
                    pass
    elif '.tar' in archive_path.name or archive_path.suffix in ['.gz', '.bz2', '.xz']:
        try:
            subprocess.run(['tar', '-xf', str(archive_path), '-C', str(tmp)], 
                         check=True, capture_output=True)
        except (subprocess.CalledProcessError, FileNotFoundError):
            with tarfile.open(archive_path, 'r:*') as tf:
                tf.extractall(tmp)
    elif archive_path.suffix == '.7z':
        try:
            subprocess.run(['7z', 'x', str(archive_path), f'-o{tmp}', '-y'],
                         check=True, capture_output=True)
        except subprocess.CalledProcessError:
            log_error(f"Failed to extract 7z archive: {archive_path}")
    else:
        log_error(f"Unsupported archive format: {archive_path}")
    
    return tmp

def get_version_from_base(base_path):
    if not base_path:
        return None
    try:
        p = Path(base_path)
        if p.is_file():
            if p.suffix == '.zip':
                with zipfile.ZipFile(p, 'r') as zf:
                    if 'version.json' in zf.namelist():
                        with zf.open('version.json') as vf:
                            data = json.loads(vf.read().decode('utf-8'))
                            return str(data.get('version', ''))
                    elif 'manifest.json' in zf.namelist():
                        with zf.open('manifest.json') as mf:
                            data = json.loads(mf.read().decode('utf-8'))
                            return str(data.get('version', ''))
            elif '.tar' in p.name or p.suffix in ['.gz', '.bz2', '.xz']:
                try:
                    for test_file in ['version.json', 'manifest.json']:
                        result = subprocess.run(
                            ['tar', '-xOf', str(p), test_file],
                            capture_output=True, text=True
                        )
                        if result.returncode == 0 and result.stdout.strip():
                            data = json.loads(result.stdout)
                            return str(data.get('version', ''))
                except:
                    with tarfile.open(p, 'r:*') as tf:
                        for member in tf.getmembers():
                            if member.name in ['version.json', 'manifest.json']:
                                f = tf.extractfile(member)
                                if f:
                                    data = json.loads(f.read().decode('utf-8'))
                                    return str(data.get('version', ''))
            elif p.suffix == '.7z':
                tmp = extract_archive(p)
                vfile = tmp / 'version.json'
                if vfile.exists():
                    return str(json.loads(vfile.read_text(encoding='utf-8')).get('version', ''))
                shutil.rmtree(tmp, ignore_errors=True)
        elif p.is_dir():
            vfile = p / 'version.json'
            if vfile.exists():
                data = json.loads(vfile.read_text(encoding='utf-8'))
                return str(data.get('version', ''))
    except Exception:
        pass
    return None

def create_tar_xz_with_external(source_dir, output_path, all_files, threads=None):
    """使用外部 tar + xz 命令创建 tar.xz，支持多线程最高压缩（管道方式）"""
    if threads is None:
        threads = get_cpu_threads()
    
    log(f"Creating tar.xz using external commands with {threads} threads (pipe mode)...")
    
    # 创建临时目录来准备所有文件
    tmp_dir = Path(tempfile.mkdtemp(prefix="tarxz_"))
    
    try:
        # 将所有文件复制到临时目录
        for rel, full in all_files:
            dest = tmp_dir / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(full, dest)
        
        # 使用管道：tar 打包 -> xz 压缩 -> 输出文件
        log("Creating and compressing tar.xz...")
        
        tar_cmd = ['tar', '-cf', '-', '-C', str(tmp_dir), '.']
        xz_cmd = ['xz', '-9', f'-T{threads}', '-']
        
        with open(output_path, 'wb') as outfile:
            tar_process = subprocess.Popen(
                tar_cmd, 
                stdout=subprocess.PIPE, 
                stderr=subprocess.PIPE
            )
            
            xz_process = subprocess.Popen(
                xz_cmd, 
                stdin=tar_process.stdout, 
                stdout=outfile, 
                stderr=subprocess.PIPE
            )
            
            tar_process.stdout.close()
            
            tar_stderr = tar_process.stderr.read()
            xz_stderr = xz_process.stderr.read()
            
            tar_exit = tar_process.wait()
            xz_exit = xz_process.wait()
            
            if tar_exit != 0:
                error_msg = tar_stderr.decode('utf-8', errors='ignore')
                log_error(f"tar failed: {error_msg}")
                raise subprocess.CalledProcessError(tar_exit, tar_cmd, stderr=tar_stderr)
            
            if xz_exit != 0:
                error_msg = xz_stderr.decode('utf-8', errors='ignore')
                log_error(f"xz failed: {error_msg}")
                raise subprocess.CalledProcessError(xz_exit, xz_cmd, stderr=xz_stderr)
        
        log(f"tar.xz created successfully: {output_path.name}")
            
    except subprocess.CalledProcessError as e:
        log_error(f"External compression failed: {e}")
        log("Falling back to Python tarfile...")
        with tarfile.open(output_path, "w:xz") as tf:
            for rel, full in all_files:
                tf.add(full, arcname=rel)
    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)

def create_archive(source_dir, output_path, format_type, all_files):
    """Create archive in specified format"""
    output_path = Path(output_path)
    
    if format_type == 'zip':
        with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zf:
            for rel, full in all_files:
                zf.write(full, arcname=rel)
    
    elif format_type == 'tar.xz':
        create_tar_xz_with_external(source_dir, output_path, all_files)
    
    elif format_type == '7z':
        threads = get_cpu_threads()
        tmp_dir = Path(tempfile.mkdtemp(prefix="pkg_"))
        try:
            for rel, full in all_files:
                dest = tmp_dir / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(full, dest)
            
            cmd = ['7z', 'a', '-mx=9', f'-mmt={threads}', str(output_path), '*']
            result = subprocess.run(cmd, cwd=str(tmp_dir), capture_output=True, text=True)
            if result.returncode != 0:
                log_error(f"7z creation failed: {result.stderr}")
                raise RuntimeError("7z compression failed")
        finally:
            shutil.rmtree(tmp_dir, ignore_errors=True)

def make_full_zip(source_dir, version, format_type='zip'):
    """Build full package in specified format"""
    ext_map = {
        'zip': '.zip',
        'tar.xz': '.tar.xz',
        '7z': '.7z'
    }
    ext = ext_map.get(format_type, '.zip')
    zip_path = DOWNLOADS_DIR / f"Lingchat_{version}{ext}"
    
    log(f"Building full package: {version} (format: {format_type})")
    
    files_data = []
    all_files = []
    
    for root, _, filenames in safe_walk(source_dir):
        for fn in filenames:
            full = Path(root) / fn
            try:
                if full.is_symlink():
                    continue
                rel = full.relative_to(source_dir).as_posix()
                all_files.append((rel, full))
            except Exception:
                continue
    
    for rel, full in all_files:
        files_data.append({"path": rel, "sha256": sha256_file(full)})
    
    manifest = {"version": version, "full": True, "files": files_data, "format": format_type}
    
    # Create manifest as temp file
    tmp_manifest = Path(tempfile.mktemp(suffix='.json'))
    tmp_manifest.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding='utf-8')
    all_files_with_manifest = all_files + [("manifest.json", tmp_manifest)]
    
    # Create archive with manifest included
    create_archive(source_dir, zip_path, format_type, all_files_with_manifest)
    
    tmp_manifest.unlink()
    
    log(f"Full package created: {zip_path.name} ({zip_path.stat().st_size / 1024 / 1024:.2f} MB)")
    return zip_path

def make_delta_zip(source_dir, base_zip_or_dir, version, base_version=None, format_type='zip'):
    """Build delta package in specified format"""
    log(f"Building delta package: {version} (format: {format_type})")
    
    ext_map = {
        'zip': '.zip',
        'tar.xz': '.tar.xz',
        '7z': '.7z'
    }
    ext = ext_map.get(format_type, '.zip')
    
    tmp_extracted = None
    if base_zip_or_dir:
        base_path = Path(base_zip_or_dir)
        if base_path.is_file():
            tmp_extracted = extract_archive(base_path)
            base_dir = tmp_extracted
        elif base_path.is_dir():
            base_dir = base_path
        else:
            base_dir = None
    else:
        base_dir = None
    
    if base_dir is None:
        log("No base provided, building full package instead")
        zip_path = make_full_zip(source_dir, version, format_type)
        if tmp_extracted:
            shutil.rmtree(tmp_extracted, ignore_errors=True)
        return zip_path
    
    src_files = collect_files(source_dir)
    base_files = collect_files(base_dir)
    
    changed = [p for p in src_files if p not in base_files or base_files[p] != src_files[p]]
    deleted = [p for p in base_files if p not in src_files]
    
    added = [p for p in changed if p not in base_files]
    modified = [p for p in changed if p in base_files]
    
    log(f"Changes - Added: {len(added)}, Modified: {len(modified)}, Deleted: {len(deleted)}")
    
    manifest = {
        "version": version,
        "base_version": base_version or get_version_from_base(base_zip_or_dir),
        "files": [{"path": rel, "sha256": src_files[rel]} for rel in changed],
        "delete": deleted,
        "format": format_type
    }
    
    # Handle no-diff case
    if not changed and not deleted:
        zip_path = DOWNLOADS_DIR / f"Lingchat_{version}_nodiff{ext}"
        
        tmp_manifest = Path(tempfile.mktemp(suffix='.json'))
        tmp_manifest.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding='utf-8')
        
        if format_type == 'zip':
            with zipfile.ZipFile(zip_path, "w") as zf:
                zf.write(tmp_manifest, arcname="manifest.json")
        
        elif format_type == 'tar.xz':
            create_tar_xz_with_external(source_dir, zip_path, [("manifest.json", tmp_manifest)])
        
        elif format_type == '7z':
            threads = get_cpu_threads()
            tmp_dir = Path(tempfile.mkdtemp(prefix="pkg_"))
            try:
                shutil.copy2(tmp_manifest, tmp_dir / "manifest.json")
                cmd = ['7z', 'a', '-mx=9', f'-mmt={threads}', str(zip_path), 'manifest.json']
                subprocess.run(cmd, cwd=str(tmp_dir), check=True, capture_output=True)
            finally:
                shutil.rmtree(tmp_dir, ignore_errors=True)
        
        tmp_manifest.unlink()
        
        if tmp_extracted:
            shutil.rmtree(tmp_extracted, ignore_errors=True)
        return zip_path
    
    zip_path = DOWNLOADS_DIR / f"Lingchat_{version}_delta{ext}"
    
    # Collect changed files for archive
    changed_files = [(rel, source_dir / Path(rel)) for rel in changed if (source_dir / Path(rel)).exists()]
    
    # Add manifest to changed files
    tmp_manifest = Path(tempfile.mktemp(suffix='.json'))
    tmp_manifest.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding='utf-8')
    all_delta_files = changed_files + [("manifest.json", tmp_manifest)]
    
    # Create archive based on format
    if format_type == 'zip':
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
            for rel, full in all_delta_files:
                zf.write(full, arcname=rel)
    
    elif format_type == 'tar.xz':
        create_tar_xz_with_external(source_dir, zip_path, all_delta_files)
    
    elif format_type == '7z':
        threads = get_cpu_threads()
        tmp_dir = Path(tempfile.mkdtemp(prefix="pkg_"))
        try:
            for rel, full in all_delta_files:
                dest = tmp_dir / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(full, dest)
            
            cmd = ['7z', 'a', '-mx=9', f'-mmt={threads}', str(zip_path), '*']
            subprocess.run(cmd, cwd=str(tmp_dir), check=True, capture_output=True)
        finally:
            shutil.rmtree(tmp_dir, ignore_errors=True)
    
    tmp_manifest.unlink()
    
    if tmp_extracted:
        shutil.rmtree(tmp_extracted, ignore_errors=True)
    
    log(f"Delta package created: {zip_path.name} ({zip_path.stat().st_size / 1024 / 1024:.2f} MB)")
    return zip_path

def write_version_json(version, zip_path, changelog="", download_url=None):
    version_json = {
        "version": version,
        "download_url": download_url,
        "sha256": sha256_file(zip_path),
        "changelog": changelog,
        "type": "delta" if "_delta" in zip_path.name else "full",
        "format": Path(zip_path).suffix.lstrip('.'),
        "timestamp": datetime.now().isoformat(),
        "size": zip_path.stat().st_size
    }
    (UPDATES_DIR / "version.json").write_text(json.dumps(version_json, indent=2, ensure_ascii=False), encoding="utf-8")
    log("Version info written")

def update_version_history(version, zip_path, changelog="", download_url=None):
    history_file = UPDATES_DIR / "history.json"
    history = {"versions": [], "latest": version}
    
    if history_file.exists():
        try:
            data = json.loads(history_file.read_text(encoding='utf-8'))
            history["versions"] = data.get("versions", [])
        except Exception:
            pass
    
    version_info = {
        "version": version,
        "download_url": download_url,
        "sha256": sha256_file(zip_path),
        "changelog": changelog,
        "type": "delta" if "_delta" in zip_path.name else "full",
        "format": Path(zip_path).suffix.lstrip('.'),
        "timestamp": datetime.now().isoformat(),
        "size": zip_path.stat().st_size
    }
    
    existing = False
    for i, v in enumerate(history["versions"]):
        if v.get("version") == version:
            history["versions"][i] = version_info
            existing = True
            break
    
    if not existing:
        history["versions"].append(version_info)
    
    history["versions"].sort(key=lambda x: x.get("timestamp", ""))
    if history["versions"]:
        history["latest"] = history["versions"][-1].get("version", version)
    
    history_file.write_text(json.dumps(history, indent=2, ensure_ascii=False), encoding="utf-8")
    log("Version history updated")

def upload_file(file_path, remote_name, server_url, api_key=None):
    if not server_url:
        return False
    
    upload_url = f"{server_url.rstrip('/')}/updates/upload"
    
    try:
        with open(file_path, 'rb') as f:
            files = {'file': (remote_name, f, 'application/octet-stream')}
            headers = {}
            if api_key:
                headers['X-API-Key'] = api_key
            response = requests.post(upload_url, files=files, headers=headers, timeout=30)
            return response.status_code in [200, 201]
    except Exception as e:
        log_error(f"Upload failed: {e}")
        return False

def upload_version_files(server_url, version, version_json_path, history_json_path, zip_path=None, api_key=None):
    if not server_url:
        log("No upload URL configured, skipping upload")
        return
    
    if version_json_path.exists():
        if upload_file(version_json_path, 'version.json', server_url, api_key):
            log("version.json uploaded successfully")
        else:
            log_error("Failed to upload version.json")
    
    if history_json_path.exists():
        if upload_file(history_json_path, 'history.json', server_url, api_key):
            log("history.json uploaded successfully")
        else:
            log_error("Failed to upload history.json")

def upload_zipfile(zip_path, ms_token, owner_name, repo_name=None):
    from modelscope.hub.api import HubApi
    api = HubApi()
    api.login(ms_token)
    zip_name = zip_path.name
    model_id = f"{owner_name}/{repo_name}"
    api.upload_file(
        path_or_fileobj=zip_path,
        path_in_repo=zip_name,
        repo_id=model_id,
        commit_message='upload update package',
    )
    download_url = f"https://modelscope.cn/models/{model_id}/resolve/master/{zip_name}"
    return download_url

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python build_package.py <version> <source_dir> [changelog] [options]")
        print("Options:")
        print("  --base <path>         Base version path for delta")
        print("  --base-version <ver>  Base version number")
        print("  --format <fmt>        Archive format: zip, tar.xz, 7z (default: zip)")
        print("  --full                Force full package")
        print("  --upload-url <url>    Upload server URL")
        print("  --upload-key <key>    Upload API key")
        print("  --ms-token <token>    ModelScope token")
        print("  --owner-name <name>   ModelScope owner")
        print("  --repo-name <name>    ModelScope repo")
        sys.exit(1)
    
    version = sys.argv[1]
    source_dir = Path(sys.argv[2])
    changelog = ""
    base = None
    force_full = False
    base_version = None
    upload_url = None
    upload_key = None
    ms_token = None
    owner_name = None
    repo_name = None
    format_type = 'zip'
    
    args = sys.argv[3:]
    i = 0
    remaining_args = []
    
    while i < len(args):
        a = args[i]
        
        if a == "--base" and i + 1 < len(args):
            base = Path(args[i+1])
            i += 2
        elif a == "--base-version" and i + 1 < len(args):
            base_version = args[i+1]
            i += 2
        elif a == "--format" and i + 1 < len(args):
            format_type = args[i+1]
            if format_type not in ['zip', 'tar.xz', '7z']:
                log_error(f"Unsupported format: {format_type}. Using zip.")
                format_type = 'zip'
            i += 2
        elif a == "--full":
            force_full = True
            i += 1
        elif a == "--upload-url" and i + 1 < len(args):
            upload_url = args[i+1]
            i += 2
        elif a == "--upload-key" and i + 1 < len(args):
            upload_key = args[i+1]
            i += 2
        elif a == "--ms-token" and i + 1 < len(args):
            ms_token = args[i+1]
            i += 2
        elif a == "--owner-name" and i + 1 < len(args):
            owner_name = args[i+1]
            i += 2
        elif a == "--repo-name" and i + 1 < len(args):
            repo_name = args[i+1]
            i += 2
        else:
            remaining_args.append(a)
            i += 1
    
    if remaining_args:
        changelog = " ".join(remaining_args)
    
    if base:
        if not base.exists():
            log_error(f"Base path does not exist: {base}")
            sys.exit(1)
    
    if format_type == '7z':
        try:
            subprocess.run(['7z', '--help'], capture_output=True, check=True)
        except (subprocess.CalledProcessError, FileNotFoundError):
            log_error("7z command not found. Please install p7zip-full")
            sys.exit(1)
    
    if format_type == 'tar.xz':
        try:
            subprocess.run(['xz', '--version'], capture_output=True, check=True)
        except (subprocess.CalledProcessError, FileNotFoundError):
            log_error("xz command not found. Please install xz-utils")
            sys.exit(1)
    
    log(f"Using {get_cpu_threads()} CPU threads for compression")
    
    try:
        if force_full:
            zip_path = make_full_zip(source_dir, version, format_type)
        else:
            zip_path = make_delta_zip(source_dir, base, version, base_version, format_type)
        
        download_url = None
        if ms_token and owner_name and repo_name:
            download_url = upload_zipfile(zip_path, ms_token, owner_name, repo_name)
        
        write_version_json(version, zip_path, changelog, download_url)
        update_version_history(version, zip_path, changelog, download_url)
        
        if upload_url:
            version_json_path = UPDATES_DIR / "version.json"
            history_json_path = UPDATES_DIR / "history.json"
            upload_version_files(upload_url, version, version_json_path, history_json_path, zip_path, upload_key)
        
        log(f"Package build completed: {zip_path.name} ({zip_path.stat().st_size / 1024 / 1024:.2f} MB)")
    
    except Exception as e:
        log_error(f"Build failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)