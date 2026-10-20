"""工具共用的超时口径。

动作回执超时明显小于工具自身的 15s，否则模型在工具超时里干等。
"""

ACTION_TIMEOUT_MS = 8000
LOOK_TIMEOUT_MS = 8000
