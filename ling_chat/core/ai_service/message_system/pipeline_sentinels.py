"""
流式管道使用的内部 sentinel 对象。

当 consumer 处理某个 index 失败、得不到合法 ReplyResponse 时，
不能简单地不写 results_store 也不 set publish_events，否则
publisher 会在该 index 上永久阻塞，导致整个流式输出 pipeline 死锁。

为此 consumer 改为写入一个 SkippedSentence 占位，并 set 事件。
publisher 拿到占位后跳过该 index；如果占位还携带 isFinal=True，
说明流已结束，publisher 直接退出循环。
"""


class SkippedSentence:
    """Sentinel for sentences that failed processing but should not block the pipeline."""

    __slots__ = ("isFinal",)

    def __init__(self, is_final: bool = False) -> None:
        self.isFinal = is_final

    def __repr__(self) -> str:  # pragma: no cover - 调试辅助
        return f"SkippedSentence(isFinal={self.isFinal})"
