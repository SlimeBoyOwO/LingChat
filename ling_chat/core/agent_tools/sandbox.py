"""
LingChat Agent 沙盒工具
提供安全的文件读写和命令执行能力，所有操作限制在沙盒目录内。
"""

import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import uuid
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any

from ling_chat.utils.runtime_path import user_data_path

# 沙盒根目录
SANDBOX_DIR = user_data_path / "sandbox"
SANDBOX_DIR.mkdir(parents=True, exist_ok=True)
APP_ROOT = Path(__file__).resolve().parents[3]
EMBEDDED_PYTHON = APP_ROOT / "python-3.13.7-embed-amd64" / "python.exe"
PACKAGE_NAME_RE = re.compile(r"^[A-Za-z0-9_.-]+$")


def _line_change_stats(before: str, after: str) -> dict[str, int]:
    before_lines = before.splitlines()
    after_lines = after.splitlines()
    added = 0
    removed = 0
    for tag, i1, i2, j1, j2 in SequenceMatcher(
        None, before_lines, after_lines
    ).get_opcodes():
        if tag == "insert":
            added += j2 - j1
        elif tag == "delete":
            removed += i2 - i1
        elif tag == "replace":
            removed += i2 - i1
            added += j2 - j1
    return {
        "lines_added": added,
        "lines_removed": removed,
        "line_count": len(after_lines),
    }


# 命令白名单（只允许这些命令及其变体）
# 格式: (命令前缀, 允许的最大参数数量)
# 注意: rm 命令在白名单内（受 DANGEROUS_PATTERNS 拦截 rm -rf /），
#      沙盒内 rm -rf ./xxx 是允许的——沙盒目录是隔离的工作区，AI 可清理临时文件
COMMAND_ALLOWLIST = {
    "python": 50,
    "python3": 50,
    "node": 20,
    "npm": 20,
    "pnpm": 20,
    "echo": 10,
    "cat": 5,
    "ls": 10,
    "dir": 10,
    "mkdir": 5,
    "rmdir": 5,
    "rm": 5,
    "cp": 5,
    "mv": 5,
    "touch": 5,
    "find": 10,
    "grep": 10,
    "head": 5,
    "tail": 5,
    "wc": 5,
    "sort": 5,
    "uniq": 5,
    "curl": 10,
    "wget": 10,
    "git": 20,
    "pip": 20,
    "pip3": 20,
}

# 禁止的危险命令/参数
DANGEROUS_PATTERNS = [
    r"\brm\s+-rf\s+/",
    r"\bformat\b",
    r"\bdd\s+if=",
    r"\bmkfs\.",
    r"\bfdisk\b",
    r":\(\)\{\s*:\|:&\};:",  # fork bomb
    r"\bshutdown\b",
    r"\breboot\b",
    r"\bpoweroff\b",
    r"\bdel\s+/[fq]",
    r"\brd\s+/s\s+/q",
]


def _resolve_sandbox_path(relative_path: str) -> Path:
    """解析沙盒内路径，防止路径遍历攻击"""
    # 规范化路径：先基于沙盒目录拼接，再 resolve，防止 .. 遍历
    # 不能直接 resolve relative_path，否则在 Windows 上 . 会变成当前工作目录的绝对路径
    raw_parts = Path(relative_path).parts
    # 过滤掉 .. 和 .，只保留正常路径组件
    safe_parts = [p for p in raw_parts if p not in (".", "..")]
    full_path = (SANDBOX_DIR / Path(*safe_parts)).resolve()
    # 安全检查：路径必须在沙盒目录下
    try:
        full_path.relative_to(SANDBOX_DIR.resolve())
    except ValueError:
        raise PermissionError(
            f"Path '{relative_path}' is outside the sandbox"
        ) from None
    return full_path


def _python_executable() -> Path:
    """返回沙盒命令优先使用的 Python。"""
    if EMBEDDED_PYTHON.exists():
        return EMBEDDED_PYTHON
    return Path(sys.executable)


def _sandbox_env() -> dict[str, str]:
    """构造隔离环境变量，让 pip 包安装和 import 都落在沙盒内。"""
    sandbox_python_path = SANDBOX_DIR / "Python"
    sandbox_appdata = SANDBOX_DIR / "AppData"
    sandbox_local_appdata = sandbox_appdata / "Local"
    sandbox_pip_cache = SANDBOX_DIR / ".pip-cache"

    sandbox_python_path.mkdir(parents=True, exist_ok=True)
    sandbox_appdata.mkdir(parents=True, exist_ok=True)
    sandbox_local_appdata.mkdir(parents=True, exist_ok=True)
    sandbox_pip_cache.mkdir(parents=True, exist_ok=True)

    return {
        "PATH": os.environ.get("PATH", ""),
        "PYTHONPATH": str(sandbox_python_path),
        "PYTHONUSERBASE": str(sandbox_python_path),
        "HOME": str(SANDBOX_DIR),
        "USERPROFILE": str(SANDBOX_DIR),
        "APPDATA": str(sandbox_appdata),
        "LOCALAPPDATA": str(sandbox_local_appdata),
        "PIP_CACHE_DIR": str(sandbox_pip_cache),
        "TEMP": str(tempfile.gettempdir()),
        "TMP": str(tempfile.gettempdir()),
    }


def _command_name(raw_command: str) -> str:
    cmd = raw_command.strip().strip('"').strip("'").lower()
    base_cmd = os.path.basename(cmd)
    if base_cmd.endswith(".exe"):
        base_cmd = base_cmd[:-4]
    return base_cmd


def _rewrite_python_command(command: str) -> str:
    parts = command.strip().split(maxsplit=1)
    if not parts:
        return command

    base_cmd = _command_name(parts[0])
    if base_cmd not in {"python", "python3"}:
        return command

    rest = f" {parts[1]}" if len(parts) > 1 else ""
    return f'"{_python_executable()}"{rest}'


def _python_runner_args(command: str) -> list[str] | None:
    parts = command.strip().split()
    if not parts or _command_name(parts[0]) not in {"python", "python3"}:
        return None

    python_exe = str(_python_executable())
    sandbox_package_path = str(SANDBOX_DIR / "Python")
    if len(parts) >= 3 and parts[1] == "-m":
        module_name = parts[2]
        module_args = parts[3:]
        code = (
            "import runpy, sys; "
            f"sys.path.insert(0, {sandbox_package_path!r}); "
            f"sys.argv = {[module_name, *module_args]!r}; "
            f"runpy.run_module({module_name!r}, run_name='__main__', alter_sys=True)"
        )
        return [python_exe, "-c", code]

    if len(parts) >= 3 and parts[1] == "-c":
        user_code = command.strip().split("-c", 1)[1].strip().strip('"').strip("'")
        code = f"import sys; sys.path.insert(0, {sandbox_package_path!r});\n{user_code}"
        return [python_exe, "-c", code]

    if len(parts) >= 2:
        script = parts[1]
        script_args = parts[2:]
        code = (
            "import runpy, sys; "
            f"sys.path.insert(0, {sandbox_package_path!r}); "
            f"sys.argv = {[script, *script_args]!r}; "
            f"runpy.run_path({script!r}, run_name='__main__')"
        )
        return [python_exe, "-c", code]

    return [python_exe]


def _truncate_output(text: str, label: str) -> str:
    max_output = 5000
    if len(text) > max_output:
        return text[:max_output] + f"\n...[{label} truncated]"
    return text


def sandbox_read_file(path: str) -> dict[str, Any]:
    """读取沙盒内文件"""
    try:
        file_path = _resolve_sandbox_path(path)
        if not file_path.exists():
            return {"ok": False, "error": f"File not found: {path}"}
        if file_path.is_dir():
            return {"ok": False, "error": f"'{path}' is a directory, not a file"}
        # 限制文件大小（10MB）
        max_size = 10 * 1024 * 1024
        if file_path.stat().st_size > max_size:
            return {"ok": False, "error": f"File too large (>10MB): {path}"}
        content = file_path.read_text(encoding="utf-8", errors="replace")
        return {
            "ok": True,
            "path": str(file_path.relative_to(SANDBOX_DIR)),
            "content": content,
            "size": len(content),
        }
    except PermissionError as e:
        return {"ok": False, "error": str(e)}
    except Exception as e:
        return {"ok": False, "error": f"Read failed: {e}"}


def _looks_like_incomplete_text(path: Path, content: str) -> str | None:
    stripped = content.rstrip()
    if not stripped:
        return "content is empty"
    if stripped.endswith((".", ",", ":", "\\", "(", "[", "{")):
        return "content ends with an unfinished token"
    return None


def _run_python_syntax_check(file_path: Path) -> dict[str, Any]:
    result = subprocess.run(
        [str(_python_executable()), "-m", "py_compile", str(file_path)],
        cwd=str(SANDBOX_DIR),
        capture_output=True,
        text=True,
        timeout=20,
        env=_sandbox_env(),
        shell=False,
        check=False,
    )
    return {
        "ok": result.returncode == 0,
        "stderr": _truncate_output(result.stderr, "stderr"),
    }


def _python_syntax_check(file_path: Path) -> dict[str, Any] | None:
    if file_path.suffix.lower() != ".py":
        return None
    return _run_python_syntax_check(file_path)


def sandbox_write_file(path: str, content: str, append: bool = False) -> dict[str, Any]:
    """Write text into a sandbox file with verification after the write."""
    try:
        file_path = _resolve_sandbox_path(path)
        file_path.parent.mkdir(parents=True, exist_ok=True)
        content = "" if content is None else str(content)
        existed_before = file_path.exists()
        previous_content = (
            file_path.read_text(encoding="utf-8", errors="replace")
            if existed_before and file_path.is_file()
            else ""
        )
        if append:
            with file_path.open("a", encoding="utf-8") as file:
                file.write(content)
        else:
            temp_file = file_path.with_name(f".{file_path.name}.{uuid.uuid4().hex}.tmp")
            try:
                temp_file.write_text(content, encoding="utf-8")
                if file_path.suffix.lower() == ".py":
                    temp_syntax_check = _run_python_syntax_check(temp_file)
                    if not temp_syntax_check.get("ok", False):
                        return {
                            "ok": False,
                            "path": str(file_path.relative_to(SANDBOX_DIR)),
                            "size": file_path.stat().st_size
                            if file_path.exists()
                            else 0,
                            "requested_size": len(content),
                            "mode": "write",
                            "verified": False,
                            "syntax_check": temp_syntax_check,
                            "error": "Python syntax check failed; existing file was preserved",
                            "content_preview": content[:160],
                        }
                temp_file.replace(file_path)
            finally:
                if temp_file.exists():
                    temp_file.unlink()

        written_content = file_path.read_text(encoding="utf-8", errors="replace")
        verified = (
            written_content == content
            if not append
            else written_content.endswith(content)
        )
        syntax_check = _python_syntax_check(file_path)
        incomplete_warning = _looks_like_incomplete_text(file_path, written_content)
        ok = verified and (syntax_check is None or syntax_check.get("ok", False))
        stats = _line_change_stats(previous_content, written_content)
        return {
            "ok": ok,
            "path": str(file_path.relative_to(SANDBOX_DIR)),
            "size": len(written_content),
            "requested_size": len(content),
            "bytes": file_path.stat().st_size,
            "mode": "append" if append else "write",
            "created": not existed_before,
            **stats,
            "verified": verified,
            "syntax_check": syntax_check,
            "incomplete_warning": incomplete_warning,
            "content_preview": written_content[:160],
        }
    except PermissionError as e:
        return {"ok": False, "error": str(e)}
    except Exception as e:
        return {"ok": False, "error": f"Write failed: {e}"}


def sandbox_list_files(path: str = ".") -> dict[str, Any]:
    """列出沙盒内文件和目录"""
    try:
        dir_path = _resolve_sandbox_path(path)
        if not dir_path.exists():
            return {"ok": False, "error": f"Directory not found: {path}"}
        if not dir_path.is_dir():
            return {"ok": False, "error": f"'{path}' is not a directory"}

        items = []
        for item in sorted(dir_path.iterdir()):
            stat = item.stat()
            items.append(
                {
                    "name": item.name,
                    "type": "directory" if item.is_dir() else "file",
                    "size": stat.st_size if item.is_file() else None,
                    "modified": stat.st_mtime,
                }
            )

        return {
            "ok": True,
            "path": str(dir_path.relative_to(SANDBOX_DIR)),
            "items": items,
        }
    except PermissionError as e:
        return {"ok": False, "error": str(e)}
    except Exception as e:
        return {"ok": False, "error": f"List failed: {e}"}


def _count_tree_items(path: Path) -> int:
    if path.is_file() or path.is_symlink():
        return 1
    count = 0
    for _root, dirs, files in os.walk(path):
        count += len(dirs) + len(files)
    return count


def _clear_directory_contents(path: Path) -> int:
    deleted = 0
    for child in list(path.iterdir()):
        deleted += _remove_path(child)
    return deleted


def _make_writable(path: Path) -> None:
    try:
        os.chmod(path, stat.S_IWRITE | stat.S_IREAD | stat.S_IEXEC)
    except OSError:
        pass


def _clear_windows_file_attributes(path: Path) -> None:
    if os.name != "nt":
        return
    subprocess.run(
        ["attrib", "-R", "-H", "-S", str(path), "/S", "/D"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        shell=False,
        check=False,
    )


def _rmtree_onerror(func, path: str, _exc_info) -> None:
    target = Path(path)
    _make_writable(target)
    try:
        func(path)
    except OSError:
        _clear_windows_file_attributes(target)
        func(path)


def _remove_path(path: Path) -> int:
    deleted = _count_tree_items(path)
    _clear_windows_file_attributes(path)
    _make_writable(path)
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path, onerror=_rmtree_onerror)
    else:
        path.unlink()
    return deleted


def sandbox_delete_file(path: str, recursive: bool = False) -> dict[str, Any]:
    """Delete a sandbox file, directory, or recursively clear a directory."""
    try:
        requested_path = (path or ".").strip() or "."
        is_sandbox_root = requested_path in {".", "./", "\\", ""}
        file_path = _resolve_sandbox_path(requested_path)
        if not file_path.exists():
            return {"ok": False, "error": f"File not found: {path}"}
        if file_path.is_dir():
            if recursive:
                if is_sandbox_root:
                    deleted = _clear_directory_contents(file_path)
                    return {
                        "ok": True,
                        "path": ".",
                        "type": "directory_contents",
                        "recursive": True,
                        "deleted_items": deleted,
                    }
                deleted = _remove_path(file_path)
                return {
                    "ok": True,
                    "path": str(file_path.relative_to(SANDBOX_DIR)),
                    "type": "directory",
                    "recursive": True,
                    "deleted_items": deleted,
                }

            try:
                file_path.rmdir()
            except OSError:
                return {
                    "ok": False,
                    "error": f"Directory is not empty: {path}. Retry with recursive=true to delete it.",
                }
            return {
                "ok": True,
                "path": str(file_path.relative_to(SANDBOX_DIR)),
                "type": "directory",
                "recursive": False,
            }

        _remove_path(file_path)
        return {
            "ok": True,
            "path": str(file_path.relative_to(SANDBOX_DIR)),
            "type": "file",
            "recursive": False,
            "deleted_items": 1,
        }
    except PermissionError as e:
        return {"ok": False, "error": str(e)}
    except Exception as e:
        return {"ok": False, "error": f"Delete failed: {e}"}


def _validate_command(command: str) -> tuple[bool, str]:
    """验证命令是否安全"""
    # 检查危险模式
    for pattern in DANGEROUS_PATTERNS:
        if re.search(pattern, command, re.IGNORECASE):
            return False, f"Dangerous command pattern detected: {pattern}"

    # 解析命令名
    cmd_parts = command.strip().split()
    if not cmd_parts:
        return False, "Empty command"

    # 处理路径前缀（如 /usr/bin/python 或 C:\...\python.exe）
    base_cmd = _command_name(cmd_parts[0])

    # 检查白名单
    if base_cmd not in COMMAND_ALLOWLIST:
        allowed = ", ".join(sorted(COMMAND_ALLOWLIST.keys()))
        return False, f"Command '{base_cmd}' is not in allowlist. Allowed: {allowed}"

    # 检查参数数量
    max_args = COMMAND_ALLOWLIST[base_cmd]
    if len(cmd_parts) - 1 > max_args:
        return False, f"Too many arguments for '{base_cmd}' (max {max_args})"

    return True, ""


def sandbox_execute_command(command: str, timeout: int = 30) -> dict[str, Any]:
    """在沙盒内安全执行命令"""
    # 环境变量控制
    is_enabled = os.environ.get("ENABLE_SANDBOX_COMMANDS", "true").lower() == "true"
    if not is_enabled:
        return {
            "ok": False,
            "error": "Sandbox command execution is disabled by administrator",
        }

    # 验证命令
    is_safe, error_msg = _validate_command(command)
    if not is_safe:
        return {"ok": False, "error": error_msg}

    try:
        # 设置超时
        timeout_val = min(max(timeout, 1), 120)  # 1-120 秒
        python_runner_args = _python_runner_args(command)
        command_to_run = python_runner_args or _rewrite_python_command(command)

        # 执行命令，限制工作目录
        result = subprocess.run(
            command_to_run,
            shell=python_runner_args is None,
            cwd=str(SANDBOX_DIR),
            capture_output=True,
            text=True,
            timeout=timeout_val,
            env=_sandbox_env(),
        )

        stdout = _truncate_output(result.stdout, "output")
        stderr = _truncate_output(result.stderr, "stderr")

        return {
            "ok": result.returncode == 0,
            "command": command,
            "resolved_command": command_to_run
            if isinstance(command_to_run, str)
            else " ".join(command_to_run),
            "returncode": result.returncode,
            "stdout": stdout,
            "stderr": stderr,
            "cwd": str(SANDBOX_DIR),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": f"Command timed out after {timeout_val} seconds"}
    except Exception as e:
        return {"ok": False, "error": f"Execution failed: {e}"}


def sandbox_install_package(package: str, timeout: int = 300) -> dict[str, Any]:
    """安装 Python 包到沙盒 Python 目录。"""
    package = package.strip()
    if not PACKAGE_NAME_RE.fullmatch(package):
        return {
            "ok": False,
            "error": "Package name may only contain letters, numbers, dot, underscore and hyphen",
        }

    try:
        timeout_val = min(max(timeout, 1), 600)
        target_dir = SANDBOX_DIR / "Python"
        target_dir.mkdir(parents=True, exist_ok=True)

        python_exe = _python_executable()
        command = [
            str(python_exe),
            "-m",
            "pip",
            "install",
            "--target",
            str(target_dir),
            package,
        ]
        result = subprocess.run(
            command,
            shell=False,
            cwd=str(SANDBOX_DIR),
            capture_output=True,
            text=True,
            timeout=timeout_val,
            env=_sandbox_env(),
        )

        return {
            "ok": result.returncode == 0,
            "package": package,
            "command": f"python -m pip install --target Python {package}",
            "resolved_command": " ".join(command),
            "returncode": result.returncode,
            "stdout": _truncate_output(result.stdout, "output"),
            "stderr": _truncate_output(result.stderr, "stderr"),
            "cwd": str(SANDBOX_DIR),
        }
    except subprocess.TimeoutExpired:
        return {
            "ok": False,
            "package": package,
            "error": f"Package install timed out after {timeout_val} seconds",
        }
    except Exception as e:
        return {
            "ok": False,
            "package": package,
            "error": f"Package install failed: {e}",
        }
