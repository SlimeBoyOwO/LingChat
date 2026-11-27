"""插件示例：脚本、网页、技能三条最小演示。"""

from plugin_host import (
    add_prompt_line,
    director_task_start,
    open_plugin_view,
    script_run,
    script_serve,
    script_stop,
)


def _script(_ctx):
    """前台跑 scripts/hello.mjs：读宿主写入的输入文件，在 out/ 里产出结果。"""
    result = script_run(
        "scripts/hello.mjs",
        files={"input.txt": "你好，插件！\n"},
    )
    if not result.get("ok"):
        return {
            "ok": False,
            "error": result.get("error") or "脚本示例运行失败",
            "detail": {
                "exit_code": result.get("exit_code"),
                "stderr": (result.get("stderr") or "")[-600:],
            },
        }
    files = result.get("files") or {}
    names = "、".join(sorted(files.keys())) or "（没有输出文件）"
    stdout = (result.get("stdout") or "").strip()[:300]
    return {
        "ok": True,
        "message": (
            f"脚本跑完了：退出码 {result.get('exit_code')}，"
            f"工作目录里的文件有 {names}；脚本输出 {stdout}"
        ),
    }


def _view(_ctx):
    """起一个零依赖本地网页，并放进应用内沙箱窗口。"""
    served = script_serve("scripts/view.mjs")
    if not served.get("ok"):
        return {"ok": False, "error": served.get("error") or "网页服务启动失败"}
    opened = open_plugin_view(
        served.get("url") or "",
        run_id=served.get("run_id"),
        title="插件示例 · 视图窗口",
        width=920,
        height=660,
    )
    if not opened.get("ok"):
        script_stop(served.get("run_id"))
        return {"ok": False, "error": opened.get("error") or "窗口打开失败"}
    return {
        "ok": True,
        "message": (
            f"网页窗口已打开：{served.get('url')}。"
            "这个页面由插件自带的脚本提供，关掉窗口会停止对应服务。"
        ),
    }


def _skill(_ctx):
    """投递一个导演任务，让它读取本插件携带的技能。"""
    result = director_task_start(
        "secret",
        {
            "system_prompt": (
                "你是 LingChat 插件示例里的导演。按注入的技能说明读取那张神秘纸条，"
                "把纸条上的话原样交回，不要自己编。"
            ),
            "prompt": (
                "读取本次声明的技能，找到纸条上的那句话，"
                "调用提交工具把它原样交回。"
            ),
            "include_recent": False,
            "skills": ["secret-note"],
            "submit": {
                "tool": "submit_secret",
                "description": "交回纸条上的话。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": "一句话说明你读了哪个技能、哪个文件",
                        },
                        "words": {
                            "type": "string",
                            "description": "纸条上的原话，一字不改",
                        },
                    },
                    "required": ["summary", "words"],
                },
                "result_field": "words",
                "summary_field": "summary",
            },
        },
    )
    if not result.get("ok"):
        return {"ok": False, "error": result.get("error") or "导演任务没能开始"}
    return {
        "ok": True,
        "message": "导演任务已投递；它读到纸条后，会通过旁白把原话报出来。",
    }


def _all(ctx):
    parts = []
    script = _script(ctx)
    parts.append(("脚本", script.get("message") or script.get("error")))
    view = _view(ctx)
    parts.append(("网页", view.get("message") or view.get("error")))
    skill = _skill(ctx)
    parts.append(("技能", skill.get("message") or skill.get("error")))
    ok = bool(script.get("ok") and view.get("ok") and skill.get("ok"))
    return {
        "ok": ok,
        "message": "；".join(f"{name}：{text}" for name, text in parts),
    }


TOOLS = {
    "showcase_script": _script,
    "showcase_view": _view,
    "showcase_skill": _skill,
    "showcase_all": _all,
}


def run(ctx):
    handler = TOOLS.get(ctx.get("tool_name") or "")
    if handler is None:
        return {"ok": False, "error": "未知的示例工具"}
    return handler(ctx)


def on_start(ctx):
    """启用/应用启动时跑一次脚本示例，把结果写进台词表，不用等模型挑工具。"""
    config = ctx.get("config") or {}
    if not config.get("auto_demo", True):
        return
    result = _script(ctx)
    if result.get("ok"):
        text = f"［插件示例］启用时脚本示例运行通过：{result.get('message') or ''}"
    else:
        text = f"［插件示例］启用时脚本示例运行失败：{result.get('error') or '未知原因'}"
    add_prompt_line("narrator", text)
