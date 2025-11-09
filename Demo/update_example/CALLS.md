# 调用方法清单（简要）

此文档汇总 updates 中主要类与函数的对外调用方法、返回值与简短说明，便于在代码中调用或在 CLI 中参考。

---

## 快速创建应用实例
- create_application(version_file: Optional[str] = None, update_url: str = "http://localhost:5000/updates", **kwargs) -> MyApplication  
  说明：从 version 文件读取版本并返回 MyApplication 实例。常用 entrypoint。

示例：
```py
app = create_application(version_file="version", update_url="http://localhost:5100/updates")
```

---

## MyApplication（updata_main.py）
属性：
- version: str — 当前版本字符串
- update_manager: UpdateManager — 底层更新管理器实例

主要方法：
- manual_check_update() -> bool  
  说明：同步检查更新。返回是否发现更新，并会在回调中设置 pending_update。

- download_update() -> bool  
  说明：下载已发现的更新。若内部未设置 update_info，会尝试使用 pending_update。

- start_update(backup: bool = False) -> bool  
  说明：下载并应用更新（先下载再应用）。backup 表示是否在应用前创建备份。若无更新信息会抛出 RuntimeError。

- apply_pending_update(backup: bool = False) -> bool  
  说明：等价于 start_update，应用 pending 更新。

- rollback() -> bool  
  说明：回滚到上次备份（调用 UpdateManager.rollback_update）。

- run()  
  说明：无效调用，会抛出 RuntimeError（库内占位）。

回调注册：MyApplication 在内部已为 update_manager 注册以下回调方法并维护内部字段：
- status_changed -> on_status_changed
- progress_changed -> on_progress_changed
- update_available -> on_update_available
- update_completed -> on_update_completed
- error_occurred -> on_error

---

## UpdateManager（updata_core.py）
构造时需传入 UpdateStrategy 实例。常用于更底层控制。

主要方法：
- start()  
  说明：启动管理器（若 auto_check=True 则开始后台自动检查循环）。

- stop()  
  说明：停止管理器与后台线程。

- check_for_updates() -> bool  
  说明：触发一次检查更新，返回是否有可用更新（并设置内部 update_info）。

- download_update() -> bool  
  说明：根据已存在的 update_info 下载更新，支持进度回调事件。

- apply_update(backup: bool = False) -> bool  
  说明：应用已下载的更新；若 backup=True，会先创建备份。

- rollback_update() -> bool  
  说明：从最近备份恢复到应用目录。

- skip_version(version: str = None)  
  说明：将某版本加入跳过列表（若不传使用当前 update_info 的版本）。

查询/状态方法：
- get_status() -> UpdateStatus
- get_progress() -> int
- get_update_info() -> Optional[Dict]
- get_error() -> Optional[str]
- is_update_available() -> bool

事件/回调：
- register_callback(event: str, callback: Callable)  
  支持事件键：'status_changed', 'progress_changed', 'error_occurred', 'update_available', 'update_completed'

---

## MyUpdateStrategy（updata_core.py）
实现具体的检查/下载/应用逻辑（网络、解压等）。

构造参数示例：
MyUpdateStrategy(current_version: str, update_url: str, app_directory: str = ".")

主要方法：
- check_update() -> Optional[Dict]  
  说明：请求 {update_url}/version.json，若发现更高版本返回 update_info（包含 version、download_url 等），否则返回 None。

- download_update(progress_callback: Optional[Callable] = None) -> str  
  说明：下载 zip 文件到临时目录，返回下载文件路径；支持 progress_callback(progress:int)。

- apply_update() -> bool  
  说明：解压并复制文件到 app_directory，根据 manifest.json 的规则（delete、full 等）执行同步或合并。完成后会尝试删除下载包。

辅助方法：
- _verify_file_hash(file_path, expected_hash) -> bool

异常：
- 方法会抛出 UpdateError（继承 Exception）以表示操作失败。

---

## 典型调用流程（示例）
1. 创建应用
```py
app = create_application(version_file="version", update_url="http://localhost:5100/updates")
```

2. 检查更新
```py
found = app.manual_check_update()
if found:
    info = app.update_manager.get_update_info()
    print("Found:", info)
```

3. 下载并应用（含备份）
```py
ok = app.start_update(backup=True)
if ok:
    print("更新完成")
else:
    print("更新失败")
```

4. 回滚
```py
app.rollback()
```

5. 直接使用底层管理器订阅事件
```py
def on_progress(p): 
    print("进度:", p)
app.update_manager.register_callback("progress_changed", on_progress)
```

---

## 文件/配置位置
- 默认 version 文件：d:\code\updatas\version 或 version.json（若存在）
- 默认 config：update_config.json（可通过 MyApplication 构造参数更改）
- 备份目录：默认 backup（可配置）

---

## 注意事项
- start_update 会在内部先调用 download_update，再调用 apply_update；若未先检查更新需先 manual_check_update。
- apply_update 与 rollback_update 会对文件系统做修改，请确保有权限与备份策略。
- 后台自动检查由 UpdateManager.start() 控制（由 MyApplication 在构造时传入 auto_check 参数）。
- 出错时会抛出 UpdateError / RuntimeError；也可以通过 register_callback 监听 'error_occurred'。

---