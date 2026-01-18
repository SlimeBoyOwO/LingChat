import os
import threading
from typing import Dict

from ling_chat.core.service_manager import service_manager

_runtime_update_lock = threading.Lock()

def apply_runtime_config_changes(new_values: Dict[str, str]) -> None:
	with _runtime_update_lock:
		for key, value in new_values.items():
			# 所有环境变量在进程内均为字符串，保持一致
			os.environ[str(key)] = str(value)

		ai_service = service_manager.ai_service
		if ai_service is not None:
			ai_service.apply_runtime_config(new_values)


