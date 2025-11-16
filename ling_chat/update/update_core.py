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
from typing import Optional, Dict, Any, Callable, List
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

# 在 UpdateStrategy 类中添加 current_version 属性
class UpdateStrategy(ABC):
    def __init__(self):
        self.current_version = "0.0.0"  # 添加默认值
    
    @abstractmethod
    def check_update(self) -> Optional[Dict[str, Any]]:
        """检查更新"""
        pass
    
    @abstractmethod
    def download_update(self, progress_callback: Optional[Callable] = None) -> str:
        """下载更新"""
        pass
    
    @abstractmethod
    def apply_update(self) -> bool:
        """应用更新"""
        pass
    
    # 新增可选方法，用于连续更新
    def download_update_chain(self, progress_callback: Optional[Callable] = None) -> List[Dict[str, Any]]:
        """下载整个更新链 - 可选实现，默认使用单版本下载"""
        # 默认实现：只下载最新版本
        if hasattr(self, '_update_info') and self._update_info and "update_chain" in self._update_info:
            update_chain = self._update_info["update_chain"]
            if update_chain:
                latest_update = update_chain[-1]
                # 临时设置为单版本信息
                original_info = getattr(self, '_update_info', None)
                self._update_info = latest_update
                try:
                    file_path = self.download_update(progress_callback)
                    # 恢复原始信息
                    self._update_info = original_info
                    return [{
                        "version": latest_update.get('version'),
                        "file_path": file_path,
                        "type": latest_update.get('type', 'delta')
                    }]
                except Exception:
                    self._update_info = original_info
                    raise
        # 回退到单版本下载
        file_path = self.download_update(progress_callback)
        return [{
            "version": getattr(self, '_update_info', {}).get('version', 'unknown'),
            "file_path": file_path,
            "type": getattr(self, '_update_info', {}).get('type', 'delta')
        }]
    
    def apply_update_chain(self, downloaded_files: List[Dict[str, Any]]) -> bool:
        """应用整个更新链 - 可选实现，默认使用单版本应用"""
        # 默认实现：只应用最新版本
        if downloaded_files:
            latest_file = downloaded_files[-1]
            self.downloaded_file = latest_file["file_path"]
            return self.apply_update()
        return False
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
        self._downloaded_files = []  # 新增：存储下载的文件列表
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
                # 检查是否是连续更新
                if "update_chain" in update_info:
                    update_chain = update_info["update_chain"]
                    # 检查是否有被跳过的版本
                    skipped_versions = self.config.get('skipped_versions', [])
                    filtered_chain = []
                    for version_info in update_chain:
                        ver = version_info.get('version')
                        if ver and ver not in skipped_versions:
                            filtered_chain.append(version_info)
                    
                    if filtered_chain:
                        update_info["update_chain"] = filtered_chain
                        update_info["update_count"] = len(filtered_chain)
                        if filtered_chain:
                            update_info["target_version"] = filtered_chain[-1]["version"]
                        self._update_info = update_info
                        self.set_status(UpdateStatus.UPDATE_AVAILABLE)
                        self._emit_event('update_available', update_info)
                        logger.info(f"发现 {len(filtered_chain)} 个待更新版本")
                        return True
                    else:
                        self._update_info = None
                        self.set_status(UpdateStatus.IDLE)
                        return False
                else:
                    # 单版本更新逻辑
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
        """下载单个更新（兼容原有接口）"""
        if not self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        # 如果是连续更新，使用新的下载方法
        if "update_chain" in self._update_info:
            return self.download_update_chain()
        
        self.set_status(UpdateStatus.DOWNLOADING)
        self.set_progress(0)
        
        def progress_callback(progress):
            self.set_progress(progress)
        
        try:
            download_path = self.strategy.download_update(progress_callback)
            self._downloaded_files = [{
                "version": self._update_info.get('version'),
                "file_path": download_path,
                "type": self._update_info.get('type', 'delta')
            }]
            self.set_progress(100)
            logger.info("下载完成: %s", download_path)
            return True
        except Exception as e:
            self.set_error(f"下载失败: {e}")
            return False
    
    def download_update_chain(self) -> bool:
        """下载整个更新链"""
        if not self._update_info or "update_chain" not in self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        update_chain = self._update_info["update_chain"]
        if not update_chain:
            self.set_error("更新链为空")
            return False
        
        self.set_status(UpdateStatus.DOWNLOADING)
        self.set_progress(0)
        
        def progress_callback(progress):
            self.set_progress(progress)
        
        try:
            # 直接调用策略的 download_update_chain 方法
            self._downloaded_files = self.strategy.download_update_chain(progress_callback)
            
            self.set_progress(100)
            logger.info(f"下载完成: 共 {len(self._downloaded_files)} 个版本")
            return True
            
        except Exception as e:
            self.set_error(f"下载失败: {e}")
            return False
    
    def apply_update(self, backup: bool = False) -> bool:
        """应用单个更新（兼容原有接口）"""
        if not self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        # 如果是连续更新，使用新的应用方法
        if "update_chain" in self._update_info:
            return self.apply_update_chain(backup=backup)
        
        if not hasattr(self, '_downloaded_files') or not self._downloaded_files:
            self.set_error("没有可用的下载文件")
            return False
        
        try:
            if backup:
                self.set_status(UpdateStatus.BACKING_UP)
                self._create_backup()
            
            self.set_status(UpdateStatus.APPLYING)
            self.set_progress(0)
            
            # 使用下载的文件
            file_info = self._downloaded_files[0]
            self.strategy.downloaded_file = file_info["file_path"]
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
    
    def apply_update_chain(self, backup: bool = False) -> bool:
        """应用整个更新链"""
        if not hasattr(self, '_downloaded_files') or not self._downloaded_files:
            self.set_error("没有可用的下载文件")
            return False
        
        try:
            if backup:
                self.set_status(UpdateStatus.BACKING_UP)
                self._create_backup()
            
            self.set_status(UpdateStatus.APPLYING)
            self.set_progress(0)
            
            # 直接调用策略的 apply_update_chain 方法
            success = self.strategy.apply_update_chain(self._downloaded_files)
            
            if success:
                self.set_status(UpdateStatus.COMPLETED)
                self.set_progress(100)
                self._emit_event('update_completed', self._update_info)
                logger.info("连续更新应用成功")
                # 更新当前版本 - 通过策略内部更新，这里不需要再设置
                return True
            else:
                self.set_error("应用更新返回失败")
                return False
            
        except Exception as e:
            self.set_error(f"应用更新失败: {e}")
            return False

    def perform_continuous_update(self, backup: bool = False) -> bool:
        """执行连续更新（下载并应用整个更新链）"""
        if not self._update_info:
            self.set_error("没有可用的更新信息")
            return False
        
        # 检查是否需要连续更新
        update_count = self._update_info.get("update_count", 1)
        if update_count > 1:
            logger.info(f"检测到 {update_count} 个待更新版本，开始连续更新")
        
        # 下载整个更新链
        if not self.download_update_chain():
            return False
        
        # 应用整个更新链
        return self.apply_update_chain(backup=backup)
    
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
                    backup_files.add(rel)
                for d in dirs:
                    reld = (root_path / d).relative_to(src).as_posix()
                    backup_dirs.add(reld)

            for root, dirs, files in os.walk(dst, topdown=False):
                root_path = Path(root)
                for f in files:
                    try:
                        rel = (root_path / f).relative_to(dst).as_posix()
                    except Exception:
                        continue
                    if "backup" in Path(rel).parts:
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
                    if "backup" in Path(reld).parts:
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
                target_root = dst / rel_root
                target_root.mkdir(parents=True, exist_ok=True)
                for fname in files:
                    sfile = Path(root) / fname
                    tfile = target_root / fname
                    try:
                        shutil.copy2(sfile, tfile)
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
    
    def skip_version_chain(self):
        """跳过整个更新链中的所有版本"""
        if not self._update_info or "update_chain" not in self._update_info:
            return
        
        update_chain = self._update_info["update_chain"]
        skipped_count = 0
        
        for version_info in update_chain:
            ver = version_info.get('version')
            if ver and ver not in self.config.get('skipped_versions', []):
                self.config.setdefault('skipped_versions', []).append(str(ver))
                skipped_count += 1
        
        if skipped_count > 0:
            self.save_config()
            logger.info(f"已跳过 {skipped_count} 个版本")
    
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
    
    def get_update_chain_info(self) -> Optional[Dict[str, Any]]:
        """获取更新链的详细信息"""
        if self._update_info and "update_chain" in self._update_info:
            return {
                "current_version": self._update_info.get("current_version"),
                "target_version": self._update_info.get("target_version"),
                "update_count": self._update_info.get("update_count", 0),
                "versions": [info.get("version") for info in self._update_info.get("update_chain", [])],
                "update_chain": self._update_info.get("update_chain", [])
            }
        return None

class MyUpdateStrategy(UpdateStrategy):
    def __init__(self, current_version: str, update_url: str, app_directory: str = "."):
        super().__init__()  # 调用父类初始化
        self.current_version = current_version  # 覆盖默认值
        self.update_url = update_url.rstrip("/")
        self.app_directory = app_directory
        self.downloaded_file = None
        self._update_info = None
        self._version_history = []  # 新增：版本历史
    
    def check_update(self) -> Optional[Dict[str, Any]]:
        """检查更新，返回需要更新的所有版本（从当前版本到最新版本）"""
        try:
            # 获取版本历史
            self._version_history = self.get_version_history()
            
            # 如果没有历史记录，回退到原来的单版本检查
            if not self._version_history:
                return self._check_single_update()
            
            # 安全地处理版本历史
            current_ver = version.parse(str(self.current_version))
            pending_updates = []
            
            for version_info in self._version_history:
                if not version_info:  # 跳过 None 值
                    continue
                    
                ver_str = version_info.get('version')
                if not ver_str:
                    continue
                
                try:
                    ver = version.parse(str(ver_str))
                    if ver > current_ver:
                        pending_updates.append(version_info)
                except Exception as e:
                    logger.warning(f"版本号解析失败 {ver_str}: {e}")
                    continue
            
            # 按版本号排序（从低到高）
            pending_updates.sort(key=lambda x: version.parse(str(x.get('version', '0.0.0'))))
            
            if pending_updates:
                # 返回需要更新的所有版本信息
                update_chain = {
                    "current_version": self.current_version,
                    "target_version": pending_updates[-1].get("version") if pending_updates else self.current_version,
                    "update_chain": pending_updates,
                    "update_count": len(pending_updates)
                }
                self._update_info = update_chain
                return update_chain
            
            return None
            
        except Exception as e:
            logger.error(f"检查更新失败: {e}")
            raise UpdateError(f"检查更新失败: {e}")
    
    def download_update(self, progress_callback: Optional[Callable] = None) -> str:
        """下载单个更新（兼容原有接口）"""
        if not self._update_info:
            raise UpdateError("请先检查更新")
        
        # 如果是连续更新，只下载最新版本（兼容模式）
        if "update_chain" in self._update_info:
            update_chain = self._update_info["update_chain"]
            if update_chain:
                latest_update = update_chain[-1]
                # 临时设置为单版本信息
                original_info = self._update_info
                self._update_info = latest_update
                try:
                    result = self._download_single_update(latest_update, progress_callback)
                    # 恢复原始信息
                    self._update_info = original_info
                    return result
                except Exception:
                    self._update_info = original_info
                    raise
            else:
                raise UpdateError("更新链为空")
        else:
            return self._download_single_update(self._update_info, progress_callback)
    
    def apply_update(self) -> bool:
        """应用单个更新（兼容原有接口）"""
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

            # 更新当前版本
            if self._update_info and "version" in self._update_info:
                self.current_version = self._update_info["version"]

            return True
        except UpdateError:
            raise
        except Exception as e:
            raise UpdateError(f"应用更新失败: {e}")
        finally:
            if tmpdir and tmpdir.exists():
                shutil.rmtree(tmpdir, ignore_errors=True)
    
    # 其他方法保持不变...
    def get_version_history(self):
        """获取版本历史"""
        try:
            resp = requests.get(f"{self.update_url}/history.json", timeout=10)
            resp.raise_for_status()
            history_data = resp.json()
            
            # 处理不同的响应格式
            if isinstance(history_data, dict):
                if "versions" in history_data:
                    return history_data["versions"]
                else:
                    # 如果返回的是单个版本信息，包装成列表
                    return [history_data]
            elif isinstance(history_data, list):
                return history_data
            else:
                logger.warning(f"未知的历史数据格式: {type(history_data)}")
                return []
                
        except Exception as e:
            logger.warning(f"获取版本历史失败: {e}")
            return []
    
    def _check_single_update(self):
        """原有的单版本检查逻辑"""
        try:
            resp = requests.get(f"{self.update_url}/version.json", timeout=10)
            resp.raise_for_status()
            update_info = resp.json()
            
            if 'version' not in update_info:
                raise UpdateError("version.json 缺少 version 字段")
            
            if version.parse(str(update_info['version'])) > version.parse(str(self.current_version)):
                # 包装成更新链格式以保持接口一致
                update_chain = {
                    "current_version": self.current_version,
                    "target_version": update_info['version'],
                    "update_chain": [update_info],
                    "update_count": 1
                }
                self._update_info = update_chain
                return update_chain
            return None
        except UpdateError:
            raise
        except Exception as e:
            raise UpdateError(f"检查更新失败: {e}")
    
    def _download_single_update(self, update_info, progress_callback=None):
        """下载单个更新包"""
        try:
            download_url = update_info.get('download_url')
            if not download_url:
                raise UpdateError("更新信息缺少 download_url")
            resp = requests.get(download_url, stream=True, timeout=30)
            resp.raise_for_status()
            total_size = int(resp.headers.get('content-length', 0))
            temp_dir = tempfile.gettempdir()
            fname = f"update_{update_info.get('version')}.zip"
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
            sha = update_info.get('sha256')
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
    
    def download_update_chain(self, progress_callback=None):
        """下载整个更新链中的所有更新包"""
        if not self._update_info or "update_chain" not in self._update_info:
            raise UpdateError("没有可用的更新信息")
        
        update_chain = self._update_info["update_chain"]
        total_files = len(update_chain)
        downloaded_files = []
        
        for i, update_info in enumerate(update_chain):
            if progress_callback:
                # 计算总体进度
                overall_progress = int((i / total_files) * 100)
                progress_callback(overall_progress)
            
            try:
                download_url = update_info.get('download_url')
                if not download_url:
                    raise UpdateError(f"版本 {update_info.get('version')} 缺少 download_url")
                
                logger.info(f"下载更新 {i+1}/{total_files}: {update_info.get('version')}")
                
                resp = requests.get(download_url, stream=True, timeout=30)
                resp.raise_for_status()
                
                total_size = int(resp.headers.get('content-length', 0))
                temp_dir = tempfile.gettempdir()
                fname = f"update_{update_info.get('version')}.zip"
                file_path = os.path.join(temp_dir, fname)
                
                downloaded = 0
                with open(file_path, 'wb') as f:
                    for chunk in resp.iter_content(chunk_size=8192):
                        if chunk:
                            f.write(chunk)
                            downloaded += len(chunk)
                            if progress_callback and total_size > 0:
                                # 文件内进度 + 总体进度
                                file_progress = int((downloaded / total_size) * (100 / total_files))
                                current_progress = int((i / total_files) * 100) + file_progress
                                progress_callback(min(current_progress, 100))
                
                # 验证文件哈希
                sha = update_info.get('sha256')
                if sha:
                    if not self._verify_file_hash(file_path, sha):
                        try:
                            os.remove(file_path)
                        except Exception:
                            pass
                        raise UpdateError(f"版本 {update_info.get('version')} 文件哈希验证失败")
                
                downloaded_files.append({
                    "version": update_info.get('version'),
                    "file_path": file_path,
                    "type": update_info.get('type', 'delta')
                })
                
            except Exception as e:
                # 清理已下载的文件
                for dl_file in downloaded_files:
                    try:
                        os.remove(dl_file["file_path"])
                    except Exception:
                        pass
                raise UpdateError(f"下载版本 {update_info.get('version')} 失败: {e}")
        
        if progress_callback:
            progress_callback(100)
        
        return downloaded_files
    
    def apply_update_chain(self, downloaded_files):
        """按顺序应用整个更新链"""
        if not downloaded_files:
            raise UpdateError("没有可用的下载文件")
        
        tmpdirs = []  # 记录所有临时目录以便清理
        current_version = self.current_version
        
        try:
            for i, dl_info in enumerate(downloaded_files):
                version_str = dl_info["version"]
                file_path = dl_info["file_path"]
                update_type = dl_info["type"]
                
                logger.info(f"应用更新 {i+1}/{len(downloaded_files)}: {current_version} -> {version_str}")
                
                tmpdir = None
                try:
                    tmpdir = Path(tempfile.mkdtemp(prefix=f"apply_{version_str}_"))
                    tmpdirs.append(tmpdir)
                    
                    # 解压更新包
                    with zipfile.ZipFile(file_path, 'r') as zf:
                        for member in zf.namelist():
                            member_path = Path(member)
                            if member_path.is_absolute() or ".." in member_path.parts:
                                logger.warning(f"跳过不安全的 zip 条目: {member}")
                                continue
                            target_path = tmpdir / member_path
                            if member.endswith('/'):
                                target_path.mkdir(parents=True, exist_ok=True)
                            else:
                                target_path.parent.mkdir(parents=True, exist_ok=True)
                                with zf.open(member) as srcf, open(target_path, "wb") as dstf:
                                    shutil.copyfileobj(srcf, dstf)
                    
                    # 读取 manifest
                    manifest = {}
                    mf = tmpdir / "manifest.json"
                    if mf.exists():
                        try:
                            manifest = json.loads(mf.read_text(encoding="utf-8"))
                        except Exception as e:
                            logger.warning(f"读取 manifest 失败: {e}")
                    
                    # 应用更新（复用原有的应用逻辑）
                    dst = Path(self.app_directory)
                    dst.mkdir(parents=True, exist_ok=True)
                    
                    # 处理删除列表
                    delete_list = manifest.get("delete") or []
                    for rel in delete_list:
                        try:
                            target = (dst / Path(rel)).resolve()
                            try:
                                if dst.resolve() not in target.parents and dst.resolve() != target:
                                    logger.warning(f"删除目标不在应用目录内，跳过: {target}")
                                    continue
                            except Exception:
                                pass
                            if target.exists():
                                if target.is_file() or target.is_symlink():
                                    target.unlink()
                                    logger.info(f"按 manifest 删除文件: {target}")
                                elif target.is_dir():
                                    shutil.rmtree(target, ignore_errors=True)
                                    logger.info(f"按 manifest 删除目录: {target}")
                        except Exception as e:
                            logger.warning(f"删除指定路径失败 {rel}: {e}")
                    
                    # 全量同步逻辑
                    pkg_type = update_type.lower()
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
                                        logger.info(f"同步删除文件: {(root_path / f)}")
                                    except Exception as e:
                                        logger.warning(f"删除文件失败 {(root_path / f)}: {e}")
                            try:
                                rel_dir = root_path.relative_to(dst).as_posix() if root_path != dst else ""
                                if "backup" in Path(rel_dir).parts:
                                    continue
                                if not any(root_path.iterdir()):
                                    try:
                                        root_path.rmdir()
                                        logger.info(f"删除空目录: {root_path}")
                                    except Exception:
                                        pass
                            except Exception:
                                pass
                    
                    # 复制文件
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
                    
                    # 更新当前版本
                    current_version = version_str
                    logger.info(f"成功更新到版本: {version_str}")
                    
                except Exception as e:
                    raise UpdateError(f"应用版本 {version_str} 失败: {e}")
            
            # 所有更新应用成功
            self.current_version = current_version
            return True
            
        finally:
            # 清理临时目录
            for tmpdir in tmpdirs:
                if tmpdir and tmpdir.exists():
                    shutil.rmtree(tmpdir, ignore_errors=True)
            # 清理下载的文件
            for dl_info in downloaded_files:
                try:
                    os.remove(dl_info["file_path"])
                except Exception:
                    pass
    
    def _verify_file_hash(self, file_path, expected_hash):
        sha256_hash = hashlib.sha256()
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                sha256_hash.update(chunk)
        return sha256_hash.hexdigest().lower() == str(expected_hash).lower()