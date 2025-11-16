import os
import time
import json
from pathlib import Path
from typing import Optional
from ling_chat.update.update_core import UpdateManager, MyUpdateStrategy


def load_version_from_file(version_path: Optional[str] = None) -> str:
    try:
        if version_path:
            vpath = Path(version_path)
        else:
            vpath = Path(__file__).parent / "version"
        if not vpath.exists():
            alt = vpath.with_suffix(".json")
            vpath = alt if alt.exists() else vpath
        if not vpath.exists():
            return "0.0"
        raw = vpath.read_text(encoding="utf-8").strip()
        try:
            data = json.loads(raw)
        except Exception:
            data = raw
        if isinstance(data, dict):
            return str(data.get("version", "0.0"))
        elif isinstance(data, str) and data:
            return data
        else:
            return "0.0"
    except Exception:
        return "0.0"


class MyApplication:
    def __init__(
        self,
        version: str,
        update_url: str = "http://localhost:5000/updates",
        app_dir: Optional[str] = None,
        config_file: str = "update_config.json",
        backup_dir: str = "backup",
        auto_check: bool = False,
        check_interval: int = 10,
    ):
        self.version = version
        app_directory = app_dir or os.path.dirname(__file__)
        strategy = MyUpdateStrategy(
            current_version=self.version,
            update_url=update_url,
            app_directory=app_directory,
        )
        self.update_manager = UpdateManager(
            strategy=strategy,
            config_file=config_file,
            backup_dir=backup_dir,
            auto_check=auto_check,
            check_interval=check_interval,
        )
        self._pending_update: Optional[dict] = None
        self.update_manager.register_callback("status_changed", self.on_status_changed)
        self.update_manager.register_callback("progress_changed", self.on_progress_changed)
        self.update_manager.register_callback("update_available", self.on_update_available)
        self.update_manager.register_callback("update_completed", self.on_update_completed)
        self.update_manager.register_callback("error_occurred", self.on_error)

    def on_status_changed(self, new_status, old_status):
        self._last_status = (old_status, new_status)

    def on_progress_changed(self, progress):
        self._last_progress = progress

    def on_update_available(self, update_info):
        self._pending_update = update_info

    def on_update_completed(self, update_info):
        try:
            self.version = str(update_info.get("version", self.version))
        except Exception:
            pass
        self._pending_update = None

    def on_error(self, error):
        self._last_error = str(error)
    def manual_check_update(self) -> bool:
        found = self.update_manager.check_for_updates()
        if found:
            self._pending_update = self.update_manager.get_update_info()
        return found

    def download_update(self) -> bool:
        if not self.update_manager.get_update_info():
            if self._pending_update:
                self.update_manager._update_info = self._pending_update
        return self.update_manager.download_update()

    def start_update(self, backup: bool = False) -> bool:
        """下载并应用更新（不会交互或打印）；若下载已完成可直接调用 apply_update。
        参数:
            backup: 是否在应用前创建备份
        返回:
            应用是否成功（True/False）
        """
        if not self.update_manager.get_update_info():
            if self._pending_update:
                self.update_manager._update_info = self._pending_update
            else:
                raise RuntimeError("没有可用的更新信息，请先 manual_check_update()")
        if not self.update_manager.download_update():
            return False
        return self.update_manager.apply_update(backup=backup)
    
    def apply_pending_update(self, backup: bool = False) -> bool:
        return self.start_update(backup=backup)

    def rollback(self) -> bool:
        return self.update_manager.rollback_update()

    def run(self):
        raise RuntimeError("[Error]无效调用")
    def start_continuous_update(self, backup: bool = False) -> bool:
        """执行连续更新（如果有多版本）"""
        if not self.update_manager.get_update_info():
            if self._pending_update:
                self.update_manager._update_info = self._pending_update
            else:
                raise RuntimeError("没有可用的更新信息，请先 manual_check_update()")
        
        # 使用新的连续更新方法
        if hasattr(self.update_manager, 'perform_continuous_update'):
            return self.update_manager.perform_continuous_update(backup=backup)
        else:
            # 回退到原有逻辑
            return self.start_update(backup=backup)
    
    def get_update_chain_info(self):
        """获取更新链信息"""
        update_info = self.update_manager.get_update_info()
        if update_info and "update_chain" in update_info:
            return {
                "current_version": update_info.get("current_version"),
                "target_version": update_info.get("target_version"),
                "update_count": update_info.get("update_count", 0),
                "versions": [info.get("version") for info in update_info.get("update_chain", [])]
            }
        return None

def create_application(
    version_file: Optional[str] = None,
    update_url: str = "http://localhost:5000/updates",
    **kwargs,
) -> MyApplication:
    version = load_version_from_file(version_file)
    return MyApplication(version=version, update_url=update_url, **kwargs)


def run_cli(*args, **kwargs):
    raise RuntimeError("[Error]无效调用")