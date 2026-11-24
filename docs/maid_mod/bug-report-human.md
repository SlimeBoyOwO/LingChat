## 人工 Bug 记录与排查

1. 目标寻路可放置方块位置有时候其实并不能放到
2. 会无限卡在 retry rounds，此时一直显示绿框
3. RETRY_ROUND 的整条链路：

4. 触发点：BuildRules.endPass（BuildRules.java:170）。只有两处会给出 RETRY_ROUND：
   - BuildRules.java:179：本层没有开格、后面也没有别的层可去、但还有 DEFERRED 的格子；
   - BuildRules.java:198：本层连续 3 遍没进展，动作管线是 DEFER_LAYER → RETRY_ROUND。
5. 统计来源：BuildLayer.measurePass（BuildLayer.java:41），在 MaidBuildController.scan 的 endPass(server, run, measurePass(run)) 调用（MaidBuildController.java:447）。retried、canRetryRound 都是在这一步拍快照的。
6. 执行入口：BuildLayer.endPass（BuildLayer.java:83）→ applyEnd（BuildLayer.java:88）→ perform 的 case RETRY_ROUND（BuildLayer.java:114）→ retryRound（BuildLayer.java:186）。
7. retryRound 里做的事（BuildLayer.java:186-197）：
   - 没有 DEFERRED 就直接返回，不会置 retried；
   - 每个 DEFERRED 走 CellFlow.retry（CellFlow.java:83），迁到 RETRYING；
   - 进入 RETRYING 时清空这一格的 triedStands / unreachable / steps / bridges / drops / hopUntil、waits / propped / navFails（CellFlow.java:137-149）；
   - 置 run.retried = true，然后 resetLayer。
8. 回到扫描：resetLayer（BuildLayer.java:242）→ setLayer（254）→ restartPass（277），把 layerPassPlaced、stallPasses 归零，游标回到层首。applyEnd 末尾设 run.cooldown = PASS_BREAK_TICKS（95），冷却完回 SCAN。

两个值得你重点看的地方：

- retryRound 的提前返回（187-189）在“没有 DEFERRED”时什么都不做，也不置 retried；而 BuildRules.java:193 的 retryAfter = canRetryRound || !stats.retried 会依赖前一个 DEFER_LAYER 当场造出 DEFERRED。如果 deferLayer（BuildLayer.java:169）因为 inPass 条件一个都没 park，就会出现「RETRY_ROUND 空转、retried 一直是 false、同样结局反复出现」。
- run.retried 是整次施工共用的一个标记（MaidBuildController.java:160），只在 retryRound 里置真，不按层重置（setLayer 没碰它）。

调试状态栏里 run.retried 为真时会带 ·已补（MaidBuildController.java:619）。如果卡住时显示的是 层:RETRY_ROUND 但没有 ·已补，那就是走进了空转那条早退路径——说明当时其实没有 DEFERRED 格子，前面的 deferLayer 也没 park 到东西。

4. 在跑酷过程可能会无限悬浮

5. 以上 BUG 已修复，但可能日后还会出现，继续关注。
