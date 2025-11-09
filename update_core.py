import os
import time
import threading
import json
import shutil
import logging
import requests
import tempfile
import hashlib
import zipfile
from pathlib import Path
from abc import ABC, abstractmethod
from typing import Optional, Dict, Any, Callable
from enum import Enum
from packaging import version

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("UpdateManager")

class UpdateStatus(Enum):
    IDLE = "idle"
    CHECKING = "checking"
    UPDATE_AVAILABLE = "update_available"
    DOWNLOADING = "downloading"
    EXTRACTING = "extracting"
    BACKING_UP = "backing_up"
    APPLYING = "applying"
    ROLLING_BACK = "rolling_back"
    COMPLETED = "completed"
    ERROR = "error"

class UpdateStrategy(ABC):
    @abstractmethod
    def check_update(self) -> Optional[Dict[str, Any]]:
        pass
    
    @abstractmethod
    def download_update(self, progress_callback: Optional[Callable] = None) -> str:
        pass
    
    @abstractmethod
    def apply_update(self) -> bool:
        pass

class UpdateError(Exception):
    pass

class UpdateManager:
    def __init__(self, 
                 strategy: UpdateStrategy,
                 config_file: str = "update_config.json",
                 backup_dir: str = "backup",
                 auto_check: bool = True,
                 check_interval: int = 3600):
        
        self.strategy = strategy
        self.config_file = config_file
        self.backup_dir = Path(backup_dir)
        self.auto_check = auto_check
        self.check_interval = check_interval
        
        self._status = UpdateStatus.IDLE
        self._update_info = None
        self._progress = 0
        self._error = None
        self._is_running = False
        self._callbacks = {
            'status_changed': [],
            'progress_changed': [],
            'error_occurred': [],
            'update_available': [],
            'update_completed': []
        }
        
        self.load_config()
    
    def load_config(self):
        self.config = {
            'last_check': 0,
            'skipped_versions': [],
            'auto_download': False,
            'backup_enabled': True
        }
        
        if os.path.exists(self.config_file):
            try:
                with open(self.config_file, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        self.config.update(data)
            except Exception as e:
                logger.warning(f"加载配置失败，使用默认配置: {e}")
    
    def save_config(self):
        try:
            with open(self.config_file, 'w', encoding='utf-8') as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
        except Exception as e:
            logger.error(f"保存配置文件失败: {e}")
    
    def register_callback(self, event: str, callback: Callable):
        if event in self._callbacks:
            self._callbacks[event].append(callback)
    
    def _emit_event(self, event: str, *args, **kwargs):
        for callback in list(self._callbacks.get(event, [])):
            try:
                callback(*args, **kwargs)
            except Exception as e:
                logger.exception(f"回调执行失败 ({event}): {e}")
    
    def set_status(self, status: UpdateStatus):
        old_status = self._status
        self._status = status
        if old_status != status:
            self._emit_event('status_changed', status, old_status)
            logger.info(f"状态变更: {old_status.value} -> {status.value}")
    
    def set_progress(self, progress: int):
        self._progress = progress
        self._emit_event('progress_changed', progress)
    
    def set_error(self, error: str):
        self._error = error
        self.set_status(UpdateStatus.ERROR)
        self._emit_event('error_occurred', error)
        logger.error(f"更新错误: {error}")
    
    def start(self):
        if self._is_running:
            return
        
        self._is_running = True
        if self.auto_check:
            self._start_auto_check()
        
        logger.info("更新管理器已启动")
    
    def stop(self):
        self._is_running = False
        logger.info("更新管理器已停止")
    
    def _start_auto_check(self):
        def auto_check_loop():
            while self._is_running:
                try:
                    now = time.time()
                    if now - self.config.get('last_check', 0) >= self.check_interval:
                        logger.debug("自动检查触发")
                        try:
                            self.check_for_updates()
                        except Exception as e:
                            logger.warning(f"自动检查失败: {e}")
                    time.sleep(max(1, min(self.check_interval, 5)))
                except Exception:
                    time.sleep(1)
        
        thread = threading.Thread(target=auto_check_loop, daemon=True)
        thread.start()
    
    def check_for_updates(self) -> bool:
        if self._status == UpdateStatus.CHECKING:
            return False
        
        self.set_status(UpdateStatus.CHECKING)
        self.set_progress(0)
        self._error = None
        
        try:
            update_info = self.strategy.check_update()
            self.config['last_check'] = time.time()
            self.save_config()
            
            if update_info:
                ver = update_info.get('version')
                if ver and ver in self.config.get('skipped_versions', []):
                    logger.info(f"版本 {ver} 被跳过")
                    self.set_status(UpdateStatus.IDLE)
                    return False
                self._update_info = update_info
                self.set_status(UpdateStatus.UPDATE_AVAILABLE)
                self._emit_event('update_available', update_info)
                return True
            else:
                self._update_info = None
                self.set_status(UpdateStatus.IDLE)
                return False
        except Exception as e:
            self.set_error(f"检查更新失败: {e}")
            return False
    
    def download_update(self) -> bool:
        if not self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        self.set_status(UpdateStatus.DOWNLOADING)
        self.set_progress(0)
        
        def progress_callback(progress):
            self.set_progress(progress)
        
        try:
            download_path = self.strategy.download_update(progress_callback)
            self.set_progress(100)
            logger.info("下载完成: %s", download_path)
            return True
        except Exception as e:
            self.set_error(f"下载失败: {e}")
            return False
    
    def apply_update(self, backup: bool = False) -> bool:
        if not self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        try:
            if backup:
                self.set_status(UpdateStatus.BACKING_UP)
                self._create_backup()
            
            self.set_status(UpdateStatus.APPLYING)
            self.set_progress(0)
            
            success = self.strategy.apply_update()
            
            if success:
                self.set_status(UpdateStatus.COMPLETED)
                self.set_progress(100)
                self._emit_event('update_completed', self._update_info)
                logger.info("更新应用成功")
                return True
            else:
                self.set_error("应用更新返回失败")
                return False
            
        except Exception as e:
            self.set_error(f"应用更新失败: {e}")
            return False
    
    def _create_backup(self):
        try:
            app_dir = getattr(self.strategy, "app_directory", None)
            if not app_dir:
                logger.warning("未指定 app_directory，跳过备份")
                return None
            src = Path(app_dir)
            if not src.exists():
                logger.warning("应用目录不存在，跳过备份: %s", src)
                return None

            backup_root = Path(self.backup_dir)
            backup_root.mkdir(parents=True, exist_ok=True)

            ts = int(time.time())
            target = backup_root / f"backup_{ts}"
            i = 0
            while target.exists():
                i += 1
                target = backup_root / f"backup_{ts}_{i}"

            backup_root_resolved = backup_root.resolve()
            src_resolved = src.resolve()

            for root, dirs, files in os.walk(src):
                root_path = Path(root)
                try:
                    root_resolved = root_path.resolve()
                except Exception:
                    root_resolved = root_path

                if backup_root_resolved == root_resolved or backup_root_resolved in root_resolved.parents:
                    dirs[:] = []
                    continue

                new_dirs = []
                for d in dirs:
                    candidate = (root_path / d)
                    try:
                        cand_resolved = candidate.resolve()
                    except Exception:
                        cand_resolved = candidate
                    if backup_root_resolved == cand_resolved or backup_root_resolved in cand_resolved.parents:
                        continue
                    new_dirs.append(d)
                dirs[:] = new_dirs

                rel_root = Path(root).relative_to(src)
                dest_root = target / rel_root
                dest_root.mkdir(parents=True, exist_ok=True)
                for fname in files:
                    sfile = Path(root) / fname
                    try:
                        if backup_root_resolved == sfile.resolve() or backup_root_resolved in sfile.resolve().parents:
                            continue
                    except Exception:
                        pass
                    dfile = dest_root / fname
                    try:
                        shutil.copy2(sfile, dfile)
                    except Exception as e:
                        logger.warning("备份时复制文件失败 %s -> %s: %s", sfile, dfile, e)

            self.config['_last_backup'] = str(target)
            try:
                self.save_config()
            except Exception:
                logger.debug("保存配置时出错（忽略）")
            logger.info("备份创建到: %s", target)
            return str(target)
        except Exception as e:
            logger.warning(f"创建备份失败: {e}")
            return None
    
    def rollback_update(self):
        self.set_status(UpdateStatus.ROLLING_BACK)
        try:
            last_backup = self.config.get('_last_backup')
            app_dir = getattr(self.strategy, "app_directory", None)
            backup_root = Path(self.backup_dir) if hasattr(self, "backup_dir") else None

            if not last_backup and backup_root and backup_root.exists():
                candidates = [p for p in backup_root.iterdir() if p.is_dir() and p.name.startswith("backup_")]
                if candidates:
                    candidates.sort(key=lambda p: p.stat().st_mtime, reverse=True)
                    last_backup = str(candidates[0])
                    self.config['_last_backup'] = last_backup
                    try:
                        self.save_config()
                    except Exception:
                        logger.debug("保存配置时出错（忽略）")

            if not last_backup or not app_dir:
                logger.warning("没有可用的备份信息，无法回滚")
                return False

            src = Path(last_backup)
            dst = Path(app_dir)

            if not src.exists():
                logger.warning("备份路径不存在，无法回滚: %s", src)
                return False

            try:
                if src.resolve() == dst.resolve():
                    logger.warning("备份路径与应用目录相同，取消回滚")
                    return False
            except Exception:
                pass

            backup_files = set()
            backup_dirs = set()
            for root, dirs, files in os.walk(src):
                root_path = Path(root)
                for f in files:
                    rel = (root_path / f).relative_to(src).as_posix()
                    # skip .git entries
                    if ".git" in Path(rel).parts:
                        continue
                    backup_files.add(rel)
                for d in dirs:
                    reld = (root_path / d).relative_to(src).as_posix()
                    if ".git" in Path(reld).parts:
                        continue
                    backup_dirs.add(reld)

            for root, dirs, files in os.walk(dst, topdown=False):
                root_path = Path(root)
                for f in files:
                    try:
                        rel = (root_path / f).relative_to(dst).as_posix()
                    except Exception:
                        continue
                    # skip paths under .git or backup folders
                    if "backup" in Path(rel).parts or ".git" in Path(rel).parts:
                        continue
                    if rel not in backup_files:
                        try:
                            (root_path / f).unlink()
                            logger.info("回滚时删除新增文件: %s", (root_path / f))
                        except Exception as e:
                            logger.warning("删除文件失败 %s: %s", (root_path / f), e)
                for d in dirs:
                    dpath = root_path / d
                    try:
                        reld = dpath.relative_to(dst).as_posix()
                    except Exception:
                        continue
                    if "backup" in Path(reld).parts or ".git" in Path(reld).parts:
                        continue
                    if reld in backup_dirs:
                        continue
                    try:
                        if not any(dpath.iterdir()):
                            dpath.rmdir()
                            logger.info("回滚时删除空目录: %s", dpath)
                    except Exception:
                        pass

            for root, dirs, files in os.walk(src):
                rel_root = Path(root).relative_to(src)
                # skip restoring .git contents
                if ".git" in rel_root.parts:
                    continue
                target_root = dst / rel_root
                target_root.mkdir(parents=True, exist_ok=True)
                for fname in files:
                    sfile = Path(root) / fname
                    # skip restoring .git files
                    if ".git" in (rel_root / fname).parts:
                        continue
                    tfile = target_root / fname
                    try:
                        shutil.copy2(sfile, tfile)
                    except PermissionError:
                        # 尝试设置为可写后重试一次（Windows 可用）
                        try:
                            os.chmod(tfile, 0o666)
                        except Exception:
                            pass
                        try:
                            shutil.copy2(sfile, tfile)
                        except Exception as e:
                            logger.warning("复制备份文件失败 %s -> %s: %s", sfile, tfile, e)
                    except Exception as e:
                        logger.warning("复制备份文件失败 %s -> %s: %s", sfile, tfile, e)

            logger.info("回滚完成，从 %s 恢复到 %s", src, dst)
            return True
        except Exception as e:
            logger.error(f"回滚失败: {e}")
            return False
        finally:
            self.set_status(UpdateStatus.IDLE)
    
    def skip_version(self, version: Optional[str] = None):
        if not version and self._update_info:
            version = self._update_info.get('version')
        
        if version is None:
            return

        # ensure we store a string version value
        version = str(version)

        if version and version not in self.config.get('skipped_versions', []):
            self.config.setdefault('skipped_versions', []).append(version)
            self.save_config()
            logger.info("已跳过版本: %s", version)
    
    def get_status(self) -> UpdateStatus:
        return self._status
    
    def get_progress(self) -> int:
        return self._progress
    
    def get_update_info(self) -> Optional[Dict[str, Any]]:
        return self._update_info
    
    def get_error(self) -> Optional[str]:
        return self._error
    
    def is_update_available(self) -> bool:
        return self._status == UpdateStatus.UPDATE_AVAILABLE

class MyUpdateStrategy(UpdateStrategy):
    def __init__(self, current_version: str, update_url: str, app_directory: str = "."):
        self.current_version = current_version
        self.update_url = update_url.rstrip("/")
        self.app_directory = app_directory
        self.downloaded_file = None
        self._update_info = None
    
    def check_update(self):
        try:
            resp = requests.get(f"{self.update_url}/version.json", timeout=10)
            resp.raise_for_status()
            update_info = resp.json()
            if 'version' not in update_info:
                raise UpdateError("version.json 缺少 version 字段")
            if version.parse(str(update_info['version'])) > version.parse(str(self.current_version)):
                self._update_info = update_info
                return update_info
            return None
        except UpdateError:
            raise
        except Exception as e:
            raise UpdateError(f"检查更新失败: {e}")
    
    def download_update(self, progress_callback=None):
        if not self._update_info:
            raise UpdateError("请先检查更新")
        try:
            download_url = self._update_info.get('download_url')
            if not download_url:
                raise UpdateError("更新信息缺少 download_url")
            resp = requests.get(download_url, stream=True, timeout=30)
            resp.raise_for_status()
            total_size = int(resp.headers.get('content-length', 0))
            temp_dir = tempfile.gettempdir()
            fname = f"update_{self._update_info.get('version')}.zip"
            self.downloaded_file = os.path.join(temp_dir, fname)
            downloaded = 0
            with open(self.downloaded_file, 'wb') as f:
                for chunk in resp.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total_size > 0:
                            progress = int((downloaded / total_size) * 100)
                            progress_callback(progress)
            sha = self._update_info.get('sha256')
            if sha:
                if not self._verify_file_hash(self.downloaded_file, sha):
                    try:
                        os.remove(self.downloaded_file)
                    except Exception:
                        pass
                    raise UpdateError("文件哈希验证失败")
            return self.downloaded_file
        except UpdateError:
            raise
        except Exception as e:
            raise UpdateError(f"下载失败: {e}")
    
    def apply_update(self):
        if not self.downloaded_file or not os.path.exists(self.downloaded_file):
            raise UpdateError("没有找到下载的更新文件")
        tmpdir = None
        try:
            tmpdir = Path(tempfile.mkdtemp(prefix="apply_"))
            with zipfile.ZipFile(self.downloaded_file, 'r') as zf:
                for member in zf.namelist():
                    member_path = Path(member)
                    if member_path.is_absolute() or ".." in member_path.parts:
                        logger.warning("跳过不安全的 zip 条目: %s", member)
                        continue
                    target_path = tmpdir / member_path
                    if member.endswith('/'):
                        target_path.mkdir(parents=True, exist_ok=True)
                    else:
                        target_path.parent.mkdir(parents=True, exist_ok=True)
                        with zf.open(member) as srcf, open(target_path, "wb") as dstf:
                            shutil.copyfileobj(srcf, dstf)

            manifest = {}
            mf = tmpdir / "manifest.json"
            if mf.exists():
                try:
                    manifest = json.loads(mf.read_text(encoding="utf-8"))
                except Exception as e:
                    logger.warning("读取 manifest 失败: %s", e)
                    manifest = {}

            dst = Path(self.app_directory)
            dst.mkdir(parents=True, exist_ok=True)

            delete_list = manifest.get("delete") or []
            for rel in delete_list:
                try:
                    target = (dst / Path(rel)).resolve()
                    try:
                        if dst.resolve() not in target.parents and dst.resolve() != target:
                            logger.warning("删除目标不在应用目录内，跳过: %s", target)
                            continue
                    except Exception:
                        pass
                    if target.exists():
                        if target.is_file() or target.is_symlink():
                            target.unlink()
                            logger.info("按 manifest 删除文件: %s", target)
                        elif target.is_dir():
                            shutil.rmtree(target, ignore_errors=True)
                            logger.info("按 manifest 删除目录: %s", target)
                except Exception as e:
                    logger.warning("删除指定路径失败 %s: %s", rel, e)

            pkg_type = (self._update_info or {}).get("type", "").lower()
            manifest_full_flag = bool(manifest.get("full"))
            do_sync = (pkg_type == "full") or manifest_full_flag

            if do_sync:
                pkg_files = set()
                for root, _, files in os.walk(tmpdir):
                    for f in files:
                        full = Path(root) / f
                        rel = full.relative_to(tmpdir).as_posix()
                        pkg_files.add(rel)

                for root, dirs, files in os.walk(dst, topdown=False):
                    root_path = Path(root)
                    for f in files:
                        rel = (root_path / f).relative_to(dst).as_posix()
                        if "backup" in Path(rel).parts:
                            continue
                        if rel not in pkg_files:
                            try:
                                (root_path / f).unlink()
                                logger.info("同步删除文件: %s", (root_path / f))
                            except Exception as e:
                                logger.warning("删除文件失败 %s: %s", (root_path / f), e)
                    try:
                        rel_dir = root_path.relative_to(dst).as_posix() if root_path != dst else ""
                        if "backup" in Path(rel_dir).parts:
                            continue
                        if not any(root_path.iterdir()):
                            try:
                                root_path.rmdir()
                                logger.info("删除空目录: %s", root_path)
                            except Exception:
                                pass
                    except Exception:
                        pass

            for root, dirs, files in os.walk(tmpdir):
                rel_root = Path(root).relative_to(tmpdir)
                target_root = dst / rel_root
                target_root.mkdir(parents=True, exist_ok=True)
                for fname in files:
                    sfile = Path(root) / fname
                    tfile = target_root / fname
                    try:
                        shutil.copy2(sfile, tfile)
                    except Exception as e:
                        raise UpdateError(f"应用文件复制失败: {sfile} -> {tfile}: {e}")

            try:
                os.remove(self.downloaded_file)
            except Exception:
                pass
            self.downloaded_file = None

            return True
        except UpdateError:
            raise
        except Exception as e:
            raise UpdateError(f"应用更新失败: {e}")
        finally:
            if tmpdir and tmpdir.exists():
                shutil.rmtree(tmpdir, ignore_errors=True)
    
    def _verify_file_hash(self, file_path, expected_hash):
        sha256_hash = hashlib.sha256()
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                sha256_hash.update(chunk)
        return sha256_hash.hexdigest().lower() == str(expected_hash).lower()