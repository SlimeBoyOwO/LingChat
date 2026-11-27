"""导演后台任务结束后，把纸条上读到的话写成旁白。"""

from plugin_host import add_prompt_line, director_task_take

SECRET = "祝你今天开心"
IGNORED = "「」『』《》〈〉“”\"'‘’。，、！!？?：:；;…—（）() "


def _normalize(text):
    return "".join(ch for ch in (text or "") if ch not in IGNORED and not ch.isspace())


def on_director_task(ctx):
    payload = ctx.get("payload") or {}
    if (payload.get("kind") or "").strip() != "secret":
        return

    status = (payload.get("status") or "").strip()
    if status == "failed":
        error = (payload.get("error") or "").strip()
        add_prompt_line(
            "narrator",
            f"［插件示例］导演没能读取纸条：{error or '没有给出原因'}。",
        )
        return
    if status != "done":
        return

    taken = director_task_take(task_id=(payload.get("taskId") or "").strip())
    if not taken.get("ok"):
        add_prompt_line(
            "narrator",
            f"［插件示例］导演的结果没能取回：{taken.get('error') or '未知原因'}。",
        )
        return

    result = taken.get("result") or {}
    words = (result.get("words") or "").strip() if isinstance(result, dict) else ""
    summary = (taken.get("summary") or "").strip()
    if _normalize(words) == SECRET:
        text = f"［插件示例］导演读到了神秘的话：「{SECRET}」。"
    else:
        text = f"［插件示例］导演没有读对神秘的话：交回的是 {words[:120]!r}。"
    if summary:
        text += f" 摘要：{summary[:120]}"
    add_prompt_line("narrator", text)
