"""事件元数据的统一形状。

每类事件在所属模块里用一个 EventSpec 登记：格式器、紧急度、有效期、合并键、
是否附带现场快照，以及需要看数据才能判定的紧急条件。`events/__init__.py` 只做汇总。
"""

DEFAULT_EVENT_TTL = 120


class EventSpec:
    def __init__(
        self,
        formatter,
        urgent=False,
        ttl=DEFAULT_EVENT_TTL,
        snapshot=False,
        merge_key="",
        severity=None,
        urgent_when=None,
    ):
        self.formatter = formatter
        self.urgent = urgent
        self.ttl = ttl
        self.snapshot = snapshot
        self.merge_key = merge_key
        self.severity = severity
        self.urgent_when = urgent_when

    def format(self, data):
        return self.formatter(data)

    def ttl_of(self, data):
        """这条事件值多少秒；ttl 登记成函数时按数据当场算（受伤按轻重分档）。"""
        return self.ttl(data) if callable(self.ttl) else self.ttl

    def is_urgent(self, data):
        return self.urgent or bool(self.urgent_when and self.urgent_when(data))

    def severity_of(self, data):
        return self.severity(data) if self.severity else 0
