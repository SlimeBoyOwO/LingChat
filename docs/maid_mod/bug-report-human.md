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

## 寻路伪代码：

```python
开始寻路：
if （目标位置低于 2 格 以上）：
  找到比目标位置低的地面；
  return 寻路到该地面之后再寻路目标位置；
else if （目标位置高于 2 格 以上）：
  if （头顶有方块挡着了且目标建筑高度比这个高，也就是说是否本格是开阔区域）：
    找到比目标位置低且开阔的地面；
    return 寻路到该地面之后再寻路目标位置；
  else：
    if （是否在正上方）：
      向上搭高到目标位置，抵达；
      return 结束；
    else:
      通过往四个方向垫高到目标位置下方或直接达到目标高度；
      return 重新开始寻路；
else：
  if not （原版寻路无法直接抵达，说明有缺口需要跑酷等）：
    return 直接用原版寻路抵达位置；
  else：
    if （A* 算法寻找跑酷可抵达）：
      return 使用A*寻找路径跑酷抵达位置；
    else：
      if （A* 算法考虑空气方块路径，搭水平搭路可抵达）
        return 使用A*寻找搭路方式抵达位置
      else：
        # 这里说明肯定有方块拦截了，需要考虑破坏方块抵达的方法
        if （A* 算法考虑跑酷+空气方块+破坏挡路路径，搭水平搭路必可抵达（毕竟是二维空间搜索）中间拦路的方块可破坏）：
          return 破坏+跑酷+搭路抵达位置；
        else:
          return 无法抵达，标记失败。
```

## 寻路伪代码（按优化思路修订）

```python
开始寻路（目标位置）：
# 原则：先廉价手段，再动地形，最后动破坏；额度不够报「要料」，不报「无路」

if （目标位置低于 2 格以上）：
  找到能直达的、比目标低或齐平的地面；
  return 寻路到该地面（重走本流程）之后，再重新规划目标位置；
  # 单次下落最多 20 格，次数不限

else if （目标位置高于 2 格以上）：
  if （本格头顶被方块挡着，不是开阔区域）：
    找到能直达的开阔地面；
    return 寻路到该地面（重走本流程）之后，再重新规划目标位置；
  else:
    if （目标在本格正上方）：
      原地起跳、脚下垫块，逐格升到目标位置；
      return 抵达；
      # 正上方优先垫柱子，不先往旁边挪
    else:
      四个方向垫高到目标下方或目标高度（缺托底先补托底）；
      return 重新开始寻路；
      # 升高与垫脚不限次数、不限块数；身上没料就报要料

else:  # 高差在 2 格以内
  # 第一层：原版寻路
  if （原版寻路能直接抵达）：
    return 直接走过去；
    # 执行中确实走不动，再降级到第二层

  # 第二层：只放开走、跑跳、落
  if （仅靠跑跳的 A* 能找到可达站位）：
    return 跑酷抵达；

  # 第三层：再放开平铺搭路（水面也允许搭）
  if （加了搭路的 A* 能找到可达站位）：
    return 搭路抵达；
    # 每趟最多平铺 8 格；不够就报要料

  # 第四层：再放开拆掉挡路的方块（只拆策略允许拆的）
  if （跑跳 + 搭路 + 破坏的 A* 能找到可达站位）：
    return 拆掉挡路方块，再跑酷或搭路抵达；

  return 无法抵达，标记失败。

# 四层共用规则
# 1 搜索范围：起点到目标的包围盒外扩 3，搜空就扩到 7、再到 11，每轮重算
# 2 搜到节点上限只报「没算完」，不当成「无路」
# 3 破坏单独计费，每块代价远高于搭路；图纸格与已盖好的面一律不许拆
# 4 水格不再当墙：能游就游过去，不能游就在水面铺桥
# 5 升高与垫脚只受「身上有没有料」约束，不设次数与块数上限；下落单次最多 20 格
# 5 分层语义以「允许哪些手段」为准；实现可以是多次放开手段的搜索，也可以把代价写成
#   按「破坏块数 → 搭路块数 → 步数」比较的字典序
```

相对原伪代码的改动：

1. 近距分支补回「原版寻路优先」，并允许执行失败后降级。
2. 正上方改为垫柱子优先，不再被侧向垫高的低代价挤到后面。
3. 补上破坏挡路方块的兜底，并限定只拆策略允许拆的方块。
4. 水不再当墙：可以游过去，也可以在水面搭桥。
5. 额度不足与无路分开报，避免把「料不够」判成「走不通」。
6. 搜索范围明确为外扩盒子，截断不当作无路。
7. 升高与垫脚不再设次数和块数上限，只看身上有没有料。
8. 下落上限从 10 格放到 20 格（默认值），次数不限。
