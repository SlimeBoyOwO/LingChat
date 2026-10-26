# LingChat 女仆桥接（Minecraft 模组）

LingChat 与《车万女仆》之间的桥。在游戏里开一个**只监听本机回环**的 WebSocket 服务，
配对仓库里的 Python 插件 `data/plugins/mc_maid_bridge/` 使用：

- 游戏里玩家说的话 → 送进 LingChat 的对话
- LingChat 里角色的回复 → 变成游戏内女仆的聊天气泡
- 世界的变化（换地形、天黑、下雨、受伤这类）→ 推给 LingChat 记进台词表
- 她身边的变化（主人走远、有东西靠近、进洞变暗、踩进水里）→ 同样记进台词表
- 玩家丢给她、放进她背包、喂她吃的东西 → 也记进台词表，她知道玩家给了什么
- 装了 Carry On 时，玩家把她抱起来 / 放下来 → 记进台词表（可选依赖，没装不影响别的）
- 玩家拿到成就（原版「进度」）→ 记进台词表，她能顺口提一句
- 主人拿出她最爱吃的东西（在 LingChat 的插件设置里填）→ 她像见到蛋糕一样凑过来讨，这一下也记进台词表
- 角色的意图 → 女仆真的走过去、坐下、站起来、躺到床上、到主人前面探路，换一种工作模式（种菜、钓鱼、打怪），
  盯上她身边的某个活物动手（连主人自己都能被点名），或者走过去挖掉指定的方块
- 她开口说话时 → 自动转头看向主人（打架、干活、睡觉时除外）
- 玩家问她「你周围有什么」→ 现取一份身边快照（只读，不改变世界）

能做的是**上面这几个动作、换一种工作模式、指定一个攻击目标、挖方块，外加一个只读的「看一眼她周围」**，没有执行指令、开面板、改配置这类能力。
唯一一处改玩法的地方是**投喂**：潜行 + 右键手里的食物会真的喂她吃一口（见下）。
动作接口不收坐标，目标只有「玩家」与「最近的床」两个语义值；挖掘只认「玩家准星指着的方向」
或方块的注册表 id，也不收坐标；查询接口同样不收坐标，任何帧里都不出现坐标。

## 环境要求

| | |
| --- | --- |
| Minecraft | 1.21.1 |
| NeoForge | 21.1.x（开发时用 21.1.172） |
| 车万女仆 | 1.5.3 或更高（必需依赖，没装会直接提示缺失） |
| Java（仅构建需要） | 21 |
| Carry On（可选） | 任意 1.21.1 NeoForge 版本；装上后才有「被抱起来 / 放下」两条事件 |

## 构建

```bash
cd extension/maid_mod
./gradlew build
```

产物在 `build/libs/lingchat_maid_bridge-0.1.0.jar`，丢进整合包的 `mods/` 目录即可。
Carry On 是编译期的可选依赖（`compileOnly`）：jar 里不打包它，没装的整合包照常加载。

### 国内网络

`maven.neoforged.net` 在部分网络下会被解析到错误的地址，直连必然失败。**不要把代理写进仓库里的
`gradle.properties`**（别人 clone 下来会连不上），而是放到用户级的 `~/.gradle/gradle.properties`：

```properties
systemProp.http.proxyHost=127.0.0.1
systemProp.http.proxyPort=7890
systemProp.https.proxyHost=127.0.0.1
systemProp.https.proxyPort=7890
```

Gradle 发行包走的是腾讯云镜像（见 `gradle/wrapper/gradle-wrapper.properties`），不受影响。
依赖下载过一次后会进本地缓存，之后 `./gradlew build --offline` 也能用。

## 代码结构

`src/main/java/com/lingchat/maidbridge/` 按职责分：

| 目录 | 管什么 |
| --- | --- |
| `action/` | 会改变世界或女仆状态的行为。`core` 是通用动作框架（走到/坐下/躺下/探路），`attack`、`mine`、`task`、`give`、`headwear`、`feed` 各自独立；`LongTaskGuard` 判断「她手上有没有活」，`WanderGuard` 管动作前后的随机游荡冷却。 |
| `bridge/` | 本机 WebSocket、信封、路由、协议常量。 |
| `chat/` | 游戏内聊天双向：玩家打字推给 LingChat，角色回复变成气泡。 |
| `voice/` | 游戏内语音：把 LingChat 的 TTS（WAV）分片广播给附近玩家，在女仆身上播放。 |
| `perception/` | 一切「观察世界」。`world` 全局采样、`nearby` 身边采样与活物短期记忆（`CreatureMemory`，两分钟，供追打与查询解释「刚才那只呢」）、`query` 按需查询与投影（关系、相连团、按需小图）、`event` 受伤/投掷物/死亡/成就/讨食、`item` 物品与背包、`carry` 抱放；`spatial` 是在它们底下共用的一层：`LocalTerrain` 按需读一片方块、`SightLine` 判看不看得见、`TerrainMemory` 记半分钟内看见过的方块、`BlockSight` 是清单里的一条，`SpatialReader` / `SpatialDiff` 把「新进眼里的岩浆与火」做成事件。`EventFrames`、`PerceptionProfiles`、`PerceptionSampler` 是这几层共用的帧外形、半径与采样门控。 |
| `maid/` | 女仆身份、查询、监控目标、状态与背包只读清单、最喜欢吃什么。 |
| `integration/` | 可选模组适配；目前只有 `carry`（Carry On）。 |
| `mixin/` | NeoForge mixin，包名被 `lingchat_maid_bridge.mixins.json` 固定，不要随意移动。 |

判定与读世界分开：`*Diff`、`*Rules`、`BackpackDiff` 是纯逻辑，能脱开 Minecraft 跑单测；`*Reader`、`MaidActuator`、`MineTargets` 才碰游戏。感知与动作读世界都经 `perception/spatial/LocalTerrain`：一次把范围里的方块状态抄成快照，之后各自只在这份快照上做判断。

## 配置

`config/lingchat_maid_bridge-common.toml`：

| 项 | 默认 | 说明 |
| --- | --- | --- |
| `bridge.enabled` | `true` | 总开关。关掉就不监听端口，用于判断问题出在模组还是插件 |
| `bridge.port` | `48920` | WebSocket 端口。改了要同步改插件里填的地址 |
| `bridge.chatBubbleEnabled` | `true` | 女仆头顶的聊天气泡 |
| `bridge.chatBoxEnabled` | `true` | 聊天栏里那行灰色消息（只有女仆主人看得到） |
| `bridge.perceptionEnabled` | `true` | 把世界的变化推给 LingChat（含被抱起来 / 放下、玩家拿到成就）。关掉就只剩说话 |
| `bridge.perceptionIntervalTicks` | `100` | 多久采一次样（tick，20 = 1 秒）。只在变化时才推 |
| `bridge.lowHealthRatio` | `0.30` | 血量低于这个比例时推一次「快撑不住了」 |
| `bridge.noticeEnabled` | `true` | 把**她身边**的变化也推给 LingChat，包括玩家给她的物品。关掉只影响「她自发注意到」，玩家问「你周围有什么」照常能答 |
| `bridge.noticeRadius` | `12` | 身边这一层扫多大范围（格），也是看得见目标的**下限**半径；看得见的还会跟车万女仆的 `MaidWorkRange` 取较大者 |
| `bridge.heardRadius` | `6` | 看不见的活物只在这个距离内按声音感知（格）。地表上走着、脚底下洞里十几格外的僵尸不再算她注意到 |

采样只在「LingChat 连着 + 女仆已绑定」时进行：没人听的时候采样没有意义。

**感知分两档（`PerceptionProfiles`）**：自动感知（周期采样、玩家开口时顺手带的那句）看得见的目标按工作范围算、看不见的只算 `heardRadius` 以内；玩家明确问「你周围有什么」时看得见放宽到 32 格。两档都只列最近 3 个活物，看不见的一律按「听到…的声音」说。被抱起 / 放下的旁白每种每个监控周期最多两条，反复搬运不会一直刷台词表。
`perceptionEnabled` 与 `noticeEnabled` 只管**主动推**；按需的 `maid_look_around` 是玩家问的，
不受这两个开关影响（只有 `bridge.enabled` 关掉、端口没在听时才连不上）。物品那几条
（丢、捡、收到、喂）跟 `noticeEnabled` 走；关掉只是不再往 LingChat 说，投喂本身照常生效。

监听地址写死 `127.0.0.1`，不提供配置项——协议本身没有鉴权，绑回环是唯一那层保护。

## 协议

JSON 文本帧，信封固定是 `{"type": ..., "request_id": ..., "data": {...}}`，
`request_id` 只出现在请求与它的响应上。消息名与社区那套桥接保持一致，所以插件端
不用感知换的是哪个模组。

| 方向 | type | 说明 |
| --- | --- | --- |
| 插件 → 模组 | `ping` | 心跳，直接在网络线程回 `pong`，不受游戏暂停影响 |
| 插件 → 模组 | `get_maid_status` | 返回当前已加载的女仆状态列表 |
| 插件 → 模组 | `maid_state` | 问她现在是死的还是活的（死后实体已不在世上，查不到） |
| 插件 → 模组 | `set_monitored_maid` | 告诉模组盯哪只女仆。**不发这条，玩家说话不会被推送** |
| 插件 → 模组 | `send_chat` | 让女仆说一句话 |
| 插件 → 模组 | `maid_action` | 让女仆做一个动作（见下） |
| 插件 → 模组 | `maid_set_task` | 给她换一个工作模式（见下） |
| 插件 → 模组 | `maid_attack` | 让她去打指定的那个活物（见下） |
| 插件 → 模组 | `maid_mine` | 让她去挖方块（见下） |
| 插件 → 模组 | `maid_give_item` | 把她背包里的东西交给玩家（见下） |
| 插件 → 模组 | `maid_headwear` | 把背包里的东西戴到她头上，或摘下来（见下） |
| 插件 → 模组 | `play_tts` | 在女仆的位置播放一句 TTS 语音（WAV base64，见下） |
| 插件 → 模组 | `maid_settle` | 这一轮说完了：动作之后的游荡冷却可以收（见下） |
| 插件 → 模组 | `set_favorite_food` | 她最喜欢吃的东西（角色设定在插件配置里）：`items` 是物品 id 数组，可省命名空间 |
| 插件 → 模组 | `maid_look_around` | 取一份她身边的详细快照（只读，见下） |
| 模组 → 插件 | `maid_state_result` | 死活状态的回执：`maid_id` `dead` `known` |
| 模组 → 插件 | `maid_action_result` | 动作的受理回执 |
| 模组 → 插件 | `maid_set_task_result` | 换工作模式的回执 |
| 模组 → 插件 | `maid_attack_result` | 攻击指令的回执 |
| 模组 → 插件 | `maid_mine_result` | 挖掘指令的回执 |
| 模组 → 插件 | `maid_give_item_result` | 交物品的回执 |
| 模组 → 插件 | `maid_headwear_result` | 戴/摘头饰的回执 |
| 模组 → 插件 | `maid_settle_result` | 游荡冷却收掉的回执 |
| 模组 → 插件 | `maid_look_around_result` | 身边快照的回帧 |
| 模组 → 插件 | `chat_message` | 玩家在游戏里说的话 |
| 模组 → 插件 | `event` | 世界的事件（见下） |
| 模组 → 插件 | `error` | 统一错误帧 |

### `send_chat`：让女仆说一句话

```json
{"type":"send_chat","data":{"maid_id":"...","message":"...","kaomoji":["(๑•̀ㅂ•́)و✧","(≧▽≦)"]}}
```

`kaomoji` 可选：这一句情绪的颜文字候选，模组随机挑一条，和台词一起显示成两枚气泡。
每一句都**替换**上一句留下的那两枚（先清空再加），所以头顶最多同时挂两枚，不会一句
叠一句；`kaomoji` 缺省或为空时只显示台词气泡，老插件不带这个字段也能正常用。
两枚的排布是**台词在左、颜文字在右**：气泡集合按到期时刻排序、渲染时第 0 枚画在右边，
所以颜文字的寿命比台词少 10 tick。

### `play_tts`：在游戏里播放一句语音

```json
{"type":"play_tts","data":{"maid_id":"...","format":"wav","audio":"<base64>"}}
```

- 只支持 **16-bit PCM WAV**（单/双声道）；mp3、24-bit、浮点 WAV 会被拒绝并记一行日志。
- 播放走 MC 的声音引擎，绑定在女仆身上：按她的位置做约 64 格线性衰减，受游戏里
  「语音」音量滑条控制；新的一句会打断上一句。躺在床上的她也照常出声——车万女仆
  睡着时会把实体设成静音，声音实例那边特意不看这个标记（那管的是她自己的叫声）。
- 与 `send_chat` 一样是单向推送，不回执；单句上限 8 MB。
- 服务端把语音按 512 KB 分片，广播给女仆 64 格内、装了本模组的玩家，各客户端自己拼装
  播放；单人游戏里房主也走同一条路。通道是可选注册，没装模组的玩家不会被拒绝加入，
  也不会收到这些包。

### `maid_action`：动作

```json
{"type":"maid_action","data":{"maid_id":"...","action":"go_to|sit|stand|lie_down|look_at|scout","target":"player|bed|ground"}}
```

`action` 与 `target` 都过白名单；认不出的 `target` 退回默认（走 = 玩家、躺 = 最近的床），
认不出的 `action` 直接拒绝。`ground` 只对 `lie_down` 有意义，其余动作给了也会退回默认。
**接口层面没有接受坐标的入口**。

`look_at` 保留在协议里只为兼容：插件已经不再把它注册成工具，她说话时会自动看向主人
（每来一句气泡转一次头，不占动作槽、不推事件）。

| 动作 | 目标 | 落地 | 受理即完成？ |
| --- | --- | --- | --- |
| `go_to` | `player`（默认） | 每 tick 写 `WALK_TARGET`，由原版寻路走过去 | 否，到位后推 `action_done` |
| `go_to` | `bed` | 找最近的空床（原版床与车万女仆的女仆床都算），走到床头那格 | 否，到位后推 `action_done` |
| `sit` | — | 坐下；坐姿下她不会自己走动 | 是 |
| `stand` | — | 站起来；正睡着会被叫醒，坐在地上躺着会被扶起来，坐在坐垫/椅子/船上会下来 | 是 |
| `lie_down` | `bed`（默认） | 先走到床边，再按**床头那一格**的睡位与睡姿躺下（不调 `startSleeping`：不会一口气回满、也不涨好感；躺着休息每 10 秒回一滴血） | 否，被打断或叫起来后推 `action_done` |
| `lie_down` | `ground` | 就地躺下：睡位按在脚下、每 tick 冻结移动；挨打、泡进液体、身下没地、被抱走时起来 | 否，她自己起来后推 `action_done` |
| `look_at` | `player` | 转头看向玩家，保持约 2 秒 | 是（到时静默收尾，不推事件） |
| `scout` | — | 每 tick 按主人朝向找正前方约 8 格、站得下的那一格过去；主人走动、转身就跟着换点 | 否，被叫停或自己撑不住时推 `action_done` |

两种躺法都按「睡位 + `Pose.SLEEPING`」两件一起走：地上的躺姿在脚下按一个睡位，YSM 的
闭眼与躺姿、车万女仆的「别乱走」都只认 `isSleeping()`。**床上的睡位一定压在床头那一格**：
车万女仆每 tick 把人吸到睡位中心，渲染又按睡位那格床的朝向把身子往床尾铺开一格半，
睡位落在床尾时她整个人会往床外错出去——所以「她脚下这一格」与「床头这一格」分开算，
床尾会顺着朝向翻到床头，踩在床的任意一半上都会被摆回床头中心。原版「睡位不是床就起来」与
车万女仆「日程不在睡觉时段就叫醒」由 `LivingEntityStopSleepingMixin` 在动作期间拦掉；
车万女仆的游泳管理器每 tick 会把不在水里的姿势改回站立，由 `EntityMaidPoseMixin` 挡住。
走路、做事或 `stand` 都会把这次动作顶掉，她随即被扶起来。
床上的休息只有躺在床上时算：每 10 秒回一滴血，起身、被打断或血满了计时清零；
就地躺下不回血——那只是换个姿势待着，不是在休息。

探路是持续状态，和躺下一样不会自己到期：主人往前走、转身，她就每 tick 换一次落点；
落点新算出来跟原来差不到 2 格时不换，免得主人站着晃视角时她来回打转。沿朝向逐格往回试
（先远后近、上下各找三格），脚下是水或岩浆、站不下人的格子都跳过，一路都是墙时退回
「跟住主人」；居家模式下只认活动范围里的格子。结束时推一条 `status=done` 的
`action_done`，`reason_code` 写原因：`low_health`（血不到四分之一）、`carried`、
`maid_busy`、`player_away`、`too_far`（离主人超过 24 格）。被新动作顶掉时照旧是
`cancelled`。

**游荡冷却**：动作在跑时，以及收尾后的一小段时间里，模组会按住车万女仆自己的「四处看看 +
随机走动」（`MaidRunOne`）。不然她走到一半会被拽去闻花、到了地方又自己走开。动作收尾后
最多再按 10 秒；插件在每轮最后一句上屏时发一条 `maid_settle` 就能提前放开，主人走远到
需要跟随时也会立刻放开。跟随、干活与寻路一概不受影响。

回执：

```json
{"type":"maid_action_result","data":{"accepted":true,"async":true,"action":"go_to","target":"player",
 "replaced_action":"","stood_up":false,"reason_code":"","reason":""}}
```

- `accepted=false` 表示**受理时就能判定失败**：附近没床、床都被占了、玩家不在同一维度、
  目标超出居家活动范围、女仆在睡觉 / 被骑 / 被拴 / 找不到。这时 `reason` 是中文原因，
  `reason_code` 是给程序看的短码。
- `async=true` 表示结果稍后由 `action_done` 给出（只有 `go_to`、`lie_down` 与 `scout` 是这种）。
- `stood_up=true` 表示为了这次动作先让她站起来了（坐着时她根本不会迈步）。
- `replaced_action` 是被这次动作顶掉的那个动作名；同一时刻**只允许一个动作**在跑，
  被顶掉的那次会收到一条 `status=cancelled` 的 `action_done`。

### `maid_settle`：这一轮说完了

```json
{"type":"maid_settle","data":{"maid_id":"..."}}
```

动作收尾后模组还会按着她自己的随机游荡一小会儿，等模型把这一轮的话说完；插件在每轮
**最后一句**上屏时发这一帧，冷却就当场放开。不发也能用：模组最多再等 10 秒，主人走远到
需要跟随时也会立刻放开。动作还在跑时这一帧不作数——说完了不等于事情办完了，收尾之后照样
再从 10 秒开始倒数。回执是 `maid_settle_result`（`maid_id`），插件不用等它。

### `maid_set_task`：换她的工作模式

```json
{"type":"maid_set_task","data":{"maid_id":"...","task":"touhou_little_maid:farm"}}
```

切的是车万女仆自己的任务（`TaskManager` 里注册的那些），所以她的界面、存档、日程表
跟玩家手动点出来的一模一样，模组这侧不留副本。`task` 可以写完整 id，也可以只写
`farm`（自动补 `touhou_little_maid:` 命名空间）；有没有这个模式由 `TaskManager` 回答，
**旁人模组加进来的任务照样能切**，不必改这里。

回执（`maid_set_task_result`）：

```json
{"data":{"accepted":true,"task":"touhou_little_maid:farm","previous_task":"touhou_little_maid:idle",
 "summary":"Plant and harvest crops","changed":true,"switch_result":"OK",
 "work_time":true,"home_mode":true,"stood_up":false,"replaced_action":"",
 "reason_code":"","reason":""}}
```

- `accepted=false` 是受理时就能判死的失败：没有这个工作模式、这个模式她现在用不了、
  女仆找不到。`reason` 是中文原因。
- `changed=false` 表示本来就在这个模式上（顺手补一次工具到手上，不会重复切）。
- `switch_result` 是车万女仆给的切换结果：`OK` / `NO_CHANGE` / `MISSING_REQUIRED_ITEM`
  / `PARTIAL_OK`。缺东西时切换仍然生效，只是她得上手的那件工具没在背包里。
- **`work_time` 与 `home_mode` 决定她此刻会不会真去干活**：她的日程表不在工作时段时
  要等到时段；跟随状态下有活时她不会主动跟着主人走（跟随行为让路），活干完或切回空闲
  才恢复，主人走远了也不影响她干活。这两件事插件会翻译给人听。
- `stood_up=true` 表示为了这次切换先让她站起来了（坐着的时候工作模式不会跑起来）。
- `replaced_action` 是被这次切换顶掉的插件动作：动作控制器每 tick 写寻路目标，与工作
  模式的寻路会互相顶，所以换工作前先把它收掉（被顶掉的那次仍会收到 `cancelled` 的
  `action_done`）。
- 真的换了模式时顺带广播一条 `maid_task_change` 事件（`task` + `summary`），插件据此在
  台词表里留一笔——工具回执只在那一轮，过一会儿她还得知道自己现在在干什么。
- 中文名不在模组里：专用服务端拿不到玩家语言，只有注册表 id 与英文摘要。

### `maid_attack`：去打那个活物

```json
{"type":"maid_attack","data":{"maid_id":"...","target":"minecraft:zombie","style":"fight","count":3}}
```

`target` 五种写法：`owner`（她主人）、`other_player`（另一个玩家）、`hostile`（最近的敌对生物）、
`animal`（最近的动物）、一个实体注册表 id（打最近的那一只），或者另一个玩家的名字
（大小写不敏感，照感知里看到的那个名字说）。插件负责把「僵尸」「zombie」翻成 id，也把
认不出的英文写法原样交过来——模组先按 `minecraft:<名字>` 认生物，再按玩家名认。

`count` 要打几只，默认 1、最多 8：按离她最近的顺序凑齐，追打队列一只一只推进，
全部打完才收工。附近不够这么多只时按找到的来，回执里 `requested` 是玩家要的数量。

`style` 两种力度：`fight`（默认，不给也是它）追上去一直打到对方倒下；`light` 只轻轻打
一下就收手——那一下的伤害固定 1 点（半点血，穿甲更少），也不会替她换武器，用来逗她抬手。
两者都能连着点好几只。

- **不走车万女仆的 `canAttack` 白名单**：主人自己、村民、动物、别的玩家都能点名。这是这个
  工具存在的意义，该打谁由玩家决定。仍会排除盔甲架与她自己的实体。
- **自己驱动，不依赖她的作息**：车万女仆的攻击行为只在「WORK 日程 + 攻击模式」下才跑，
  而且目标有效范围按居家/跟随模式算，稍远就清掉。所以这里不复用它那套，而是在
  `MaidAttackController` 里每 tick 推进：写 `WALK_TARGET` 追过去、贴身之后 `doHurtTarget`，
  出手间隔按她的攻击速度算。玩家半夜喊一句也能打。
- **顺手换武器**：背包里有带攻击力的东西就换到手上（和车万女仆自己的攻击模式同一口径），
  没有就空手打，回执里 `weapon=false`，插件会说明伤害很低。
- **目标范围**：找目标看 32 格（用来区分「没有」与「太远」）。同名玩家在同一个世界里、
  只是超出 32 格时，回 `target_too_far` 并点名说明。居家模式下目标要在她的活动
  范围里，否则当场拒绝（`target_too_far`）；跟随模式不再限制「主人到目标」的距离——追打期间
  跟随行为的传送被 `LongTaskGuard` 拦住了（见「已知边界」），打完收工后跟随恢复。
- **收工**：全部目标打完、被打断（新动作、换工作模式）、被抱起来，或当前这只追满 30 秒。
  她打死目标时由死亡旁白交代「被你杀了」，这里不重复。
- **打不了的原因**：附近找不到目标时先查短期记忆（见 `CreatureMemory`），回执按原因拒绝：
  `target_killed_by_maid`（她自己已经打倒了）、`target_killed_by_owner`（主人先打死了）、
  `target_killed_by_other`（别的生物先打死了）、`target_dead`（死了，死因不明）、
  `target_gone`（刚才还在附近，现在找不到）。说得清就不会退回 `no_target`，模型才改得了计划。
- **追到一半没了**：目标消失、换维度或被抱走时推 `attack_failed`（见事件表），
  理由与上面同一套；她自己打死的算打完，不推事件。剩下的目标继续打。
- **重复调用同一个目标**：追打已经在进行时只回执、不重启，`already_fighting=true`。模型
  容易把「还在打」读成「要再叫一次」，重复启动会把这一仗从头再来；回执与插件文案都要
  说清「她自己会打到对方倒下，不用再叫」。
- 回执（`maid_attack_result`）：`accepted`、`weapon`、`style`、`already_fighting`、
  `requested`、`targets[{id,type,category,name,distance,owner}]`；`target` 仍是第一只，
  老插件照读。

### `maid_mine`：挖掉方块

```json
{"type":"maid_mine","data":{"maid_id":"...","target":"looked","count":1}}
{"type":"maid_mine","data":{"maid_id":"...","target":"minecraft:iron_ore","count":1,"at":"@4,-2,-3"}}
{"type":"maid_mine","data":{"maid_id":"...","target":"","at":"@-2,0,4","whole":true}}
{"type":"maid_mine","data":{"maid_id":"...","target":"","area":"@-3,0,2 @3,3,4"}}
```

`target` 两种写法：`looked`（默认，玩家准星指着的方向）或一个方块注册表 id
（`minecraft:stone`，插件负责把「石头」这类中文翻成 id）。也接受方块标签
（`#minecraft:logs`，插件把「木头」这类泛称翻成它）。`count` 是这一次挖几块，
默认 1，上限 64。可选字段 `at` 是相对地址（见下）；`whole` 与 `area` 是两种「拆一片」
的写法（见下），给了它们就按区域拆，`count` 不再起作用。

- **玩家指哪片就在哪片挖**：准星射线放宽到 32 格，落点当作区域中心（不必凑到交互距离内）。
  写了方块名时，先在这一带 8 格内找这种方块，找到就只在这一带挖；`looked` 则先挖落点那块，
  再接着在这一带找同种方块。准星没指着、或者指着的那片没有目标时，才退回以她为中心找
  （24 格）。
- **按族找**：`#minecraft:logs` 这类标签按方块标签匹配（哪片林子都能砍），普通矿石 id
  自动带上深层变体（`iron_ore` 也认 `deepslate_iron_ore`），不会因为地下只剩深层矿而
  报「附近没有」。
- **可以指到具体一格（2026-10-22）**：`at` 写成 `@dx,dy,dz`（东、上、南，相对她脚下那一格），
  由 `maid_look_around` 的关系句给出，插件原样转回来。填了就只挖那一格（`count>1` 时以它
  为中心接着找同类）。受理前核三件事：那一格是不是要挖的方块（标签与深层矿石同一口径）、
  她此刻看见或半分钟内看见过（`SightLine` + `TerrainMemory`）、挖不挖得动与够不够得着。
  对不上回 `bad_address` / `not_loaded` / `not_that_block` / `not_seen`，回执带 `scene`
  （那一格的 id、方位、挡路）与 `nearest`（她看见过的最近同类，含它自己的 `at`），
  模型照着新地址改一次就行，不用原样重试。`target` 留空或写 `looked` 时按那一格原本的
  方块挖。
- **整团拆（2026-10-26）**：`whole=true` 时以 `at`（没给 `at` 就用准星落点）那一格为种子，
  按六面相连把同种方块收成一团，逐块拆掉——与「看一眼周围」的成团口径一致。种子那一格
  要她看见过、挖得动、不受保护；团里受保护与缺工具的方块在受理时先剔掉。一次最多 64 块，
  没数完就是 `truncated=true`。
- **按区域拆（2026-10-26）**：`area` 写两个相对角，用空格或 `~` 分开，如
  `"@-3,0,2 @3,3,4"`；两个角不分先后，每条边最多 16 格（`bad_address` 之外不偷偷缩小）。
  这一片里的空气、液体、挖不动的、受保护与缺工具的方块都不进清单；剩下的按离她的远近
  取前 64 块，多出来的只拆最近的并标 `truncated`。掉落照旧进她的背包。

- **自己驱动，不占任务槽**：和攻击一样不走车万女仆的任务/AI，`MaidMineController` 每 tick
  推进——找一块站得下、够得着、看得见、寻路走得到的落脚点，走过去，然后朝方块挖。
  她此刻在种地、钓鱼还是闲着都不影响。
- **埋着的也能挖**：目标前面挡着方块时（矿脉在石头里、树干心在树皮里），她先朝目标
  打一条射线，把先撞上的那块挖穿，再看下一块，直到目标露出来。挡路的方块不计入块数，
  一次最多挖穿 24 块（`path_limit`）就收手。原来只认「有落脚点、看得见」的方块，
  于是「看一眼周围」明明列着深层铁矿石、挖的时候却回「附近没有能挖到的这种方块」。
- **按原版速度挖**：进度按方块硬度与手上工具算，正确工具是 `速度 / 硬度 / 30`、用错工具或
  徒手是 `/100`，只有硬度为负的方块（基岩）挖不动。裂纹按 `destroyBlockProgress` 发给
  客户端，玩家看得见进度，不会瞬破。
- **自己换工具**：从背包里挑挖这一块最快、且能挖出掉落的工具换到手上（木镐不会顶掉钻石镐，
  剑也不会被拿去挖石头）。方块要求正确工具而她没有时当场拒绝（`wrong_tool`），`reason`
  会写清缺哪种镐子（按原版工具阶梯判）、她手上现在拿的是什么、脚边地上有没有现成的工具
  ——地上那把不算给过她，得先放进背包。不要求工具的方块徒手也挖，只是慢。
  每挖穿一块扣一点耐久，挖断了就换背包里下一把。她的头饰栏不是存货，那一格里的东西
  不会被当成工具换到手上。
- **掉落进她的背包**：走车万女仆自己的 `destroyBlock`（带 `canEntityDestroy` 与
  NeoForge 的实体破坏事件），受保护的方块不挖；掉落物直接进背包，装不下才掉在地上。
- **不受作息与工作模式限制**：玩家半夜喊一句也能挖。
- **收工**：挖够、附近没有同类方块、走不到、超时（单块 60 秒）都会推一条
  `maid_mine_done`（见下），不会留下「她到底挖完没有」的悬案。

回执（`maid_mine_result`）：

```json
{"data":{"accepted":true,"async":true,"looked":false,"addressed":false,"at":"","count":3,
 "block":"minecraft:stone","name":"Stone","requires_tool":true,"has_tool":true,
 "tool":"minecraft:iron_pickaxe","replaced_action":"","reason_code":"","reason":""}}
```

- `accepted=false` 是受理时就能判死的失败：没有这种方块、指着的那片和她附近都没有能挖到的、
  方块挖不动、受保护、没有能挖它的工具、玩家不在这里。`reason` 是中文原因。走不到跟前不再
  当场拒绝——她会一路挖穿挡路的方块过去。
- `async=true`：受理只代表开始挖了。挖穿 / 挖够 / 走不到由 `maid_mine_done` 事件交代，
  字段是 `status`（`done` / `partial` / `failed`）、`mined`、`wanted`、`block`、
  `area`、`remaining`、`truncated`、`stuck_at`、`reason_code`、`reason`。
  `remaining` 是清单里还剩几块（单块/若干块那条按 `wanted - mined`），`stuck_at` 是停下时
  卡着的那一格的相对地址（正常收工为空串）。**插件在那之前不能说她挖到了。**
- `has_tool=false` 表示她手上和背包里都没有趁手的工具，只能徒手挖（会慢一些）。
- `scene` 只在已经定到目标的失败里带：`id` / `name` 是那一格实际是什么，`dx`/`dy`/`dz`
  相对她脚下那一格，`at` 是它的相对地址，`facing` 同八向方位，`blockers` / `blocker`
  是视线中间隔着的实心方块数量与第一种，`blocked_above` 是正上方有没有实心方块，
  `stand_room` 是周围五格内有没有能站人的空格（只看空间，不问寻路）。没有目标的失败
  （如 `no_target`）不带，插件不要据此编方位。
- `nearest` 只在 `not_that_block` 里带：她看见过的最近同类方块，字段与 `scene` 相同，
  `at` 可以直接拿去做下一次调用的地址；她也一个都没看见过时不带。
- `addressed=true` 表示这次是按 `at` 指的：回执多带 `at` 与 `scene`（被指的那一格），
  插件说「朝你指的那块去了」而不是「去挖 N 块」。
- `area=true` 表示这次是整团或区域拆除：回执多带 `cells`（这次要拆几块）与 `truncated`
  （还有没装进这次范围的）。`block` 只在这片方块种类一致时才有值，混着几种为空串；
  `count` 与 `cells` 相同。插件说「开始拆这一片」，不说「去挖 N 块」。
- 与攻击、动作、换工作模式互斥：新的挖掘令会顶掉它们，它们也会顶掉挖掘。

### `maid_place`：放下一块方块

```json
{"type":"maid_place","data":{"maid_id":"...","item":"minecraft:stone"}}
{"type":"maid_place","data":{"maid_id":"...","item":"minecraft:oak_planks","at":"@2,0,1","facing":"north"}}
```

- `item` 是方块注册表 id（插件把中文名翻成 id）；实际取的是它的物品形态，必须在她主手、副手或背包里，头饰栏那一格不算。
- **两种定位**：没有 `at` 时按主人准星看到的那一面放，和玩家自己右键一致——点到草、雪这类可替换方块就是把它换掉。给了 `at` 就把那一格当目标（相对她脚下那一格的偏移，插件可从「看一眼周围」的地址往旁边推一格）。
- **按原版规则放**：用 `BlockPlaceContext` 加一个只用来算朝向的假玩家，台阶的上下半、原木朝向、含水、连接状态都交给游戏算；`facing` 可选 `north` / `south` / `east` / `west`，不给就用她自己的朝向。
- **受理前核**：目标格可替换（`not_replaceable`）、旁边有能贴着的面（`no_support`）、那一格没有别的活物（`occupied`）、她身上有这种方块（`no_item`）、在活动范围里（`out_of_restriction`）。走不到、材料在中途没了、放不进去由控制器收尾（`unreachable` / `timeout` / `no_item` / `place_failed` / `no_sight` / `carried` / `unconscious`）。
- 材料只从她身上扣一块，不凭空生成；放下去之后才扣，放失败不扣。
- 回执（`maid_place_result`）：`accepted`、`async`、`looked`、`addressed`、`at`、`item`、`block`、`name`、`count`、`facing`、`replaced_action`、`reason_code`、`reason`；已定到目标的失败带 `scene`（目标格的 id、方位、上方压没压着、周围站不站得下）。
- **收尾**：异步推 `maid_place_done`，字段是 `status`（`done` / `failed`）、`block`、`item`、`reason_code`、`reason`。受理只代表她过去了，插件在收到这条之前不能说她放好了。
- 与攻击、动作、挖掘、换工作模式互斥：新的放置令会顶掉它们，它们也会顶掉放置。

### `maid_give_item`：把东西交给玩家

```json
{"type":"maid_give_item","data":{"maid_id":"...","item":"minecraft:iron_pickaxe","count":1}}
```

- 只从她背包里拿（玩家放进去的、她捡起来的）；她手上正在用的工具不算。
- 玩家不在同一个维度、或者离她超过 8 格时拒绝（`player_away` / `too_far`）。
- 直接从背包里扣掉，放进玩家背包；装不下的掉在玩家脚下。她身上没有这件东西回
  `not_found`；物品 id 认不出来回 `bad_item`。这两种失败都带 `available`（她背包里的
  `{item,count}`，最多 12 种），模型下一次往返就能照着 id 改口，不用先猜。
  不知道她背包里有什么时，也可以先调 `maid_look_around`（回帧里有同一份 `backpack`）。
- 回执（`maid_give_item_result`）：`accepted`、`item`、`name`、`count`（实际给了几件）、
  `requested`（要了几件）、`reason_code`、`reason`。给不足时不补事件，插件按两个数字
  自己说「她身上只有这么多」。

### `maid_headwear`：头上顶东西

```json
{"type":"maid_headwear","data":{"maid_id":"...","action":"put","item":"minecraft:oak_planks"}}
{"type":"maid_headwear","data":{"maid_id":"...","action":"take_off"}}
```

- 她背包的**最后一格是头饰栏**（`MaidBackpackHandler.BACKPACK_ITEM_SLOT`，索引 5）：
  那一格里的东西顶在头上显示——放一块方块就是顶着一块方块。车万女仆自己会把这一格同步成
  她的 `BACKPACK_ITEM_SHOW`。
- `action=put` 时从背包里拿一件（`item` 是注册表 id）戴上去；`action=take_off`（或
  `item` 为空）把头上的摘下来放回背包。换下来的旧头饰优先放回刚腾出来的那格，没有空位就
  整件事撤掉并回 `no_space`，不会把东西弄丢。
- 头饰栏不算背包存货：`maid_give_item` 的 `available` 清单、挖矿换工具都跳过这一格；
  `maid_look_around` 的 `backpack` 也把它排除，另用 `headwear` 单独给出。
- 物品被车万女仆的黑名单禁止放进背包时回 `not_allowed`（和女仆界面同一个口径）。
- 回执（`maid_headwear_result`）：`accepted`、`action`、`item`、`name`、`headwear`
  （操作之后头上是什么，空串表示没戴）、`reason_code`、`reason`。

### 喂她吃饭：潜行 + 右键食物

车万女仆自己只认潜行喂金苹果 / 附魔金苹果，普通食物右击只会打开她的界面。本模组补上另一半：

- **潜行 + 右键任意食物**：食物先放进她空着的那只手，她照车万女仆自己吃饭的方式
  `startUsingItem`，模型把手里那件东西送向嘴边，吃完才真的消耗；随后按营养值回一点血、
  冒一串爱心（原版女仆吃完东西的样子），并推一条 `item_fed`。两只手都占着时退回当场吃掉、
  不出动画，不为一口饭把她手里的活计挪走；她正举着盾或已经在吃时这一下先不喂。
- **两侧都取消**：这个手势在 `InteractMaidEvent` 的 `HIGHEST` 就被取消，客户端与服务端
  都要拦——车万女仆的切坐姿（`LOWEST`）不判侧别，而右键实体时客户端也会本地跑一遍
  `mobInteract`；只拦服务端的话，模型会在客户端翻成坐姿，服务端却还当她站着。拦下之后
  别的附属模组的右键逻辑同样收不到，拿着食物潜行出手只会喂饭。
- **中途没吃完**：她放下那件东西（换手、动作顶掉）或等不到吃完事件时，这一口会掉回她
  脚下，不会白扣玩家的东西。
- **金苹果 / 附魔金苹果**：仍走车万女仆自己的 `ApplyGoldenAppleEvent`（成就照旧），本模组
  只在 `HIGHEST` 留一份副本，等她**真的吃完**（`MaidAfterEatEvent`）才补记一条；交互被
  别的模组取消、她没吃到，就不会误报。
- **空手与搬运时不碰**：空手右键在 Carry On 那边是抱起/放下；玩家手里抱着任何东西时也不喂、
  不取消交互，免得和搬运模组抢同一个手势。她骑着别的东西时同样不喂。
- **创造的玩家**：食物不真的扣，效果与回血照旧。
- 她睡着时右键不喂；普通右键（不潜行）不受影响，仍然打开界面。
- **她自己吃**：车万女仆的进食系统（受伤后从背包找东西吃）完成时，模组推一条
  `maid_ate`，与玩家喂食分开说。判定用 `LivingEntityUseItemEvent.Finish`，因为
  `MaidAfterEatEvent` 只给吃完剩下的那份，普通食物那里是空的，认不出吃的是什么。

### `event`：世界的事件

只在**变化显著**或**状态未初始化**时推，不是每次采样都推。字段是原始的游戏 id
与数值，中文措辞由插件那侧决定（游戏语义留在 Java，自然语言理解留在 Python）。

这是感知信息的**唯一出口**：插件把这些帧写成旁白，不往玩家发言后面追加括号之类的
处境说明（见 `docs/maid_mod/mc-maid-bridge-plan.md` 的「铁律与约定」）。

| `event_type` | 触发 | 主要字段 |
| --- | --- | --- |
| `state_snapshot` | 刚接上 / 刚绑定女仆（含重连） | `dimension` `biome` `sky` `phase` `raining` `thundering` 双方血量 `posture` |
| `biome_change` | 换了生物群系（一分钟内只报一次） | `from_biome` `biome` |
| `dimension_change` | 换了维度（同一帧不再报群系） | `from_dimension` `to_dimension` |
| `time_phase_change` | 昼夜翻转（只推翻转那一次） | `phase` = `day` / `night` |
| `weather_change` | 开始 / 停止下雨、打雷 | `raining` `thundering` |
| `low_health` | 玩家或女仆血量跌破阈值（边沿触发） | `who` `name` `health` `max_health` |
| `maid_hurt` | 女仆挨打（一小段连续挨打合成一条，两次上报至少隔 5 秒） | `damage` `count` `attacker` `damage_type` `health` `max_health`（后两项是这一波最后一下之后的血量） |
| `projectile_hit` | 雪球、鸡蛋这类零伤害的投掷物砸中了她（一小段合成一条，两次上报至少隔 5 秒；倒下与昏迷时不报） | `item` `count` `thrower` `thrower_type` `thrower_owner` |
| `creature_death` | 附近（`noticeRadius` 内）有生物死亡（10 秒内只报第一条） | `type` `category` `distance` `attacker` `attacker_type` `damage_type` `victim_owner` `attacker_owner` `attacker_maid` |
| `maid_death` | **她自己死了**：不吃死亡静默期，也不受距离限制 | `attacker` `attacker_type` `damage_type` `attacker_owner` `attacker_maid` |
| `owner_death` | **她主人死了**：不看距离与维度，主人复活后她会跟过去 | `attacker` `attacker_type` `damage_type` `attacker_maid` `home_mode`（居家模式下她不跟过去） |
| `maid_revive` | **她通过胶片复活了**：监控目标改到新 uuid 的那只 | — |
| `player_advancement` | 主人拿到了成就（原版「进度」；配方解锁这类没有展示信息的跳过） | `advancement` `title` `frame` = `task` / `goal` / `challenge` |
| `maid_carried` | 她被 Carry On 抱了起来（没装那个模组就不会出现） | — |
| `maid_dropped` | 她被放了下来 | — |
| `action_done` | 动作收尾：成功、失败或被新动作顶掉 | `action` `target` `status` `reason_code` `reason` `elapsed_ticks` `distance` |
| `attack_failed` | 追打没能打完：目标走了、被谁先打死了，或她被抱走了（她自己打死的算打完，不报） | `reason_code` = `target_left_range` / `killed_by_owner` / `killed_by_other` / `killed_unknown` / `maid_carried` / `timeout`，`killed_by_type`（凶手实体 id，玩家就是 `minecraft:player`；环境致死为空串），`remaining`，`target{id,type,category,name,distance,owner}` |
| `player_proximity_change` | 与主人的远近跨档（回滞，5 秒内只报一次；插件只在跨到 `gone` 或从 `gone` 回来时写旁白） | `from_proximity` `proximity` = `close` / `near` / `far` / `gone`，`distance` |
| `player_sight_change` | 主人进出视线（要连续两次采样一致才认） | `visible` |
| `threat_near` | 附近**从无到有**敌对生物，或最近那只由远转近 | `type` `distance` `direction` `visible` |
| `living_near` | 身边来了别的活物（主人以外的生物、别的玩家、别的女仆） | `type` `category` `distance` `direction` |
| `shelter_change` | 头顶换了档：露天 / 半遮 / 屋内（一分钟内只报一次，要连续两次采样一致） | `overhead`（`open` / `partial` / `enclosed`）、`overhead_cover`（头顶那块方块的 id，扫不到为空串）、`overhead_band`（`low` / `high`），以及给老插件用的 `sheltered` 与同键合并取最后一帧的 `tick` |
| `light_change` | 光线跨档：很黑（0-3）/ 昏暗（4-7）/ 明亮（8-15）（一分钟内只报一次） | `band`（`dark` / `dim` / `bright`） |
| `ground_change` | 脚下换了种值得说的地（火、雪、沙、耕地、冰）；坐在载具里不报；一分钟内只报一次，火除外 | `block` `kind` |
| `liquid_change` | 她自己这一格里泡着液体（水、岩浆或模组液体），或头也浸进去游泳；一分钟内只报一次 | `liquid` `from_liquid` `swimming` |
| `hazard_near` | 视野里**新出现**的岩浆、火、灵魂火、岩浆块或细雪（8 格内、露在外面、真能看见；脚下那一列交给 `ground_change` 与 `liquid_change`） | `block` `distance` `direction` |
| `vehicle_change` | 坐进船 / 矿车这类载具，或从上面下来 | `vehicle` `from_vehicle` |
| `posture_change` | 她换了姿势：坐下、站起、就地躺下、上床睡着、醒过来 | `posture` `from_posture` = `standing` / `sitting` / `sleeping` / `lying` |
| `structure_change` | 走进或离开村庄、矿井、要塞这类结构（冷静期一分钟） | `structure` `from_structure` |
| `item_pickup` | 她真的把地上的物品收进了背包 | `item` `count` `from_owner` `source` `source_id` `source_name` |
| `item_received` | 她身上多出物品（多半是玩家在女仆界面里放的）：背包、双手、盔甲栏、饰品栏都算 | `items[{item,count,slot}]`（`slot`：`hand` / `armor` / `bauble` / `backpack`） `from_owner` |
| `item_taken` | 她身上少了物品：主人在女仆界面里拿走了（只在主人 8 格内、她自己没在干活时报；她吃掉的、交出去的、放方块用掉的那几件事先扣账） | `items[{item,count,slot}]`（`slot` 同上） |
| `item_fed` | 玩家喂她吃了东西（连喂几口合成一条） | `items[{item,count}]` `leftover[{item,count}]`（吃完剩下的碗这类容器，车万女仆会把它收进她的背包） |
| `maid_ate` | 她自己从背包里找东西吃了（受伤后进食；停口 6 秒才算一顿，合并成一条） | `items[{item,count}]` `leftover[{item,count}]` |
| `favorite_food` | 主人拿着她最爱吃的东西，她的讨食姿态真的翻过去了（两次上报至少隔 10 秒） | `item` |

身边这一层 `threat_near`、踩到火上的 `ground_change` 与陷进岩浆里的 `liquid_change` 算紧急，
插件会当场请角色开口；
`favorite_food` 与 `maid_hurt` 一档，同样当场开口；
`projectile_hit` 也当场开口（被雪球砸一下总得有个反应），但有效期只有 10 秒；
`hazard_near` 不紧急（她还没踩进去），同键合并只留最近的一处，走「攒够几条」那套；
其余（含 `shelter_change`、`posture_change`、普通地面变化）同样走「攒够几条」那套。
同一批采样里推出去的帧按**危险 > 与主人相关 > 新奇**排好再发：危险是受伤、血量告急、敌对生物、
边上的岩浆与火；与主人相关是他走远或离开视线；其余换群系、进结构、落脚变化都算新奇。
事件自身的有效期、合并键与冷静期不变，插件按到达顺序写旁白。
`maid_hurt` 的有效期按伤势分档（轻伤 2 秒、中等 20 秒、重伤一分钟）：擦破皮的事等她下一次
开口时早就不算数了，重伤则要一直记着。
`creature_death` 里死的是她主人时也当场开口；`maid_death`、`owner_death` 与 `maid_revive`
一律当场开口——她倒下之后所有指令都只会回「找不到女仆」，这几条是插件唯一的信息来源。
主人死掉走单独的 `owner_death`：`creature_death` 有距离限制，而她可能被留在家里或另一个
维度，主人复活后她照样会跟过去。
`maid_death` 会先清掉挨打那条还没推出去的累计：致命那一下也是 `LivingDamageEvent`，
不丢就会「先报死、十 tick 后再报伤」，模型会以为自己还活着。
物品那几条另有一套：收到玩家给的东西、被喂饭当场开口，玩家丢下的东西被她捡起来也当场
开口；她自己捡到杂物、背包里多了东西、她自己吃了东西，都只记旁白。
**地上的掉落物本身不报**：玩家随手丢下、怪物掉的、挖方块崩出来的，她没捡就与她无关。
`item_pickup` 的 `source` 是 `owner` / `player` / `mob` / `block` / `unknown`（`player`
时 `source_name` 是那个玩家在 MC 里的名字，`mob` / `block` 时 `source_id` 是生物或方块的
注册表 id）。来源优先看掉落物身上盖的戳（见下）；老存档里已经在地上的东西没有戳，才退回
「原版 thrower + 最近 30 秒内她附近的死亡 / 方块破坏」去猜。
**掉落物出生时就被盖来源戳**：玩家丢出去的（`player`，带 uuid 与名字）、方块崩出来的
（`block`，带方块 id）、生物掉的（`mob`，带生物 id，玩家死亡掉出来的按 `player` 记）。
戳写在实体的持久化 NBT `LingChatSource` 里，跟着实体一起存盘；主人就在旁边也不会把
挖方块崩出来的东西说成「主人丢下的」。

每一帧都带 `maid_id`。没有天空的维度（下界 / 末地）不报天气与昼夜——那里这两样没有意义。

`creature_death` 的三个归属标记：`victim_owner` 死的是她主人，`attacker_owner` 是她主人动的手，
`attacker_maid` 是她自己动的手；都不是才看 `attacker_type`（生物）或 `attacker`（别的玩家）。
账号名不外发，中文说法由插件定。

### 被抱起来（Carry On，可选）

Carry On 抱人时会把实体**从世界里摘掉**，把它的完整 NBT 存在玩家身上，所以这段时间用
uuid 查不到她。本模组每 tick 反过来问「谁手里的那份 NBT 是她」：出现 → `maid_carried`，
消失 → `maid_dropped`。没装 Carry On 时一次也不碰它的类，其余功能不受影响。

被抱着的时候她不在世界里：聊天气泡、动作、「看一眼周围」都用不了，要等她被放下来。
动作、攻击、挖掘指令在这段时间直接回 `maid_carried`；控制器也不会为了让她站起来而把她从
玩家身上叫下来——那会把搬运数据和实体状态拆开，玩家看到的是「黏在手上放不下来」。
放下来时实体按原 NBT 重建（uuid 保留），感知会照常重推一条 `state_snapshot`。

### 玩家的成就

监听 NeoForge 的 `AdvancementEarnEvent`，只报她主人拿到的、且有展示信息的进度（配方解锁
这类技术性条目没有展示信息，自动跳过）。帧里给注册表 id、游戏内标题与 `task` / `goal` /
`challenge` 三档；中文名在插件的 `ADVANCEMENT_NAMES` 里，原版条目都收录了，模组加的
用游戏自己给的标题。

`action_done` 的 `status` 是 `done` / `failed` / `cancelled`，**三种都会推**：不推的话插件会
永远等一个不来的收尾。`elapsed_ticks` 是游戏内的 tick 数（Esc 暂停时不计），`distance` 是
收尾时到目标的直线距离（取一位小数）。

就地躺下的收尾特殊：她自己起来时是 `status=done`，`reason_code` 是打断那件事
（`hurt` / `in_liquid` / `lost_ground` / `carried` / `in_bed`），`reason` 是中文原因；
被新动作顶掉时照旧是 `cancelled`。

写请求响应时有两件事必须守住：响应要把 `request_id` 原样带回，而推送帧**绝不能**
带 `request_id`——LingChat 宿主按 `request_id` 认领响应，误带会让推送帧被当成某个
请求的应答吞掉。

### `maid_look_around`：看一眼她周围

```json
{"type":"maid_look_around","data":{"maid_id":"...","profile":"query","detail":"brief"}}
```

- `profile` 缺省或 `query`：玩家明确问「你周围有什么」，看得见的目标放宽到 32 格，并附
  `blocks` / `backpack`。`profile=auto`：玩家开口时顺手带的那份近处快照，看得见按工作范围、
  看不见只听 `heardRadius` 以内，不扫方块与背包（省掉每次开口的方块读取）。
- `detail` 缺省或 `brief`：只给关系与清单。`detail=map`：另附一张近细远粗的小图，
  只在玩家问「怎么过去」「带我去」时用；`auto` 档忽略它。

回帧（`maid_look_around_result`）：

```json
{"data":{
  "maid_id":"...",
  "dimension":"minecraft:overworld",
  "looking_at":{"kind":"block","id":"minecraft:oak_door","distance":4.3},
  "standing_on":{"id":"minecraft:farmland","footing":"farmland"},
  "sheltered":false, "has_sky":true, "light":12,
  "overhead":"open", "overhead_cover":"", "overhead_band":"",
  "owner":{"name":"NoiQing_Ling","distance":3.4,"visible":true,"same_dimension":true},
  "nearby":[{"type":"minecraft:zombie","category":"hostile","distance":6.8,"direction":"left","visible":false,"name":""},
            {"type":"minecraft:player","category":"player","distance":7.2,"direction":"right","visible":true,"name":"Layeray"}],
  "blocks":["minecraft:spruce_log","minecraft:dirt","minecraft:stone"],
  "block_sight":[{"id":"minecraft:spruce_log","seen":"now"},
                 {"id":"minecraft:dirt","seen":"now"},
                 {"id":"minecraft:stone","seen":"before"}],
  "relations":[{"id":"minecraft:spruce_log","seen":"now","dx":0,"dy":0,"dz":3,
                "facing":"front","same_layer":true,"distance":3.0,
                "blockers":0,"blocker":"","walk_steps":3}],
  "clusters":[{"id":"minecraft:spruce_log","cells":6,"truncated":false,
               "dx":-2,"dy":0,"dz":4,"facing":"front_left","distance":4.5,
               "blockers":0,"blocker":""}],
  "map":{"radius":8,"text":"近处（一格一单元）：\n. . .\n"},
  "backpack":[{"item":"minecraft:rose_bush","count":2},{"item":"minecraft:iron_pickaxe","count":1}],
  "headwear":{"item":"minecraft:oak_planks","count":1},
  "remembered":[{"type":"minecraft:zombie","category":"hostile","name":"","dead":true,
                 "killed_by":"maid","distance":5.2}],
  "vehicle":"", "structure":"minecraft:village_plains",
  "posture":"standing",
  "truncated":false
}}
```

- 一律只给注册表 id：专用服务端上 `getName()` 只有英文（甚至裸翻译键），中文只在客户端
  按玩家语言选得出来，所以中文名全在插件那侧定。
- `overhead` 是头顶这一层的三档：`open` 露天、`partial` 半遮（树荫、屋檐、洞口）、
  `enclosed` 屋内（房间、洞穴），口径见已知边界。`overhead_cover` 是头顶第一块挡路的方块，
  她那一格就露天时为空串；`overhead_band` 只说它压得低（`low`）还是很高（`high`），不报格数。
  没有天空的维度整项为空串。老插件继续读 `sheltered`，它仍然只是「她那一格看不看得见天」。
- `posture` 取 `standing` / `sitting` / `sleeping` / `lying`。站着是常态、插件不念；坐着、
  睡着与躺在地上要说一句，否则模型会当她站着指挥。`lying` 只可能是就地躺下——床上睡着的
  给 `sleeping`。
- `looking_at.kind` 取 `block` / `entity` / `none`；方块用 OUTLINE 射线，草丛、火把、
  告示牌这些「看得见但没碰撞」的也算挡着（与视线判定的口径刻意不同：玻璃挡得住这条
  射线，挡不住视线）。
- `blocks` 只列她**此刻看得见**、或者半分钟内看见过（还记得）的方块；竖直范围从她脚下
  这一层往上。露出来只是候选：埋在地下、四面被石头包住的不算，看得见却隔着墙的也不算。
- `direction` 是相对她**身体**朝向的八向：`front` `front_right` `right` `back_right`
  `back` `back_left` `left` `front_left`。她不一定是面朝哪就看哪，而「她周围有什么」
  说的是她站着的方位；只有 `looking_at` 那条射线用头部朝向。
- `visible` 是「她看不看得见」，判据是无遮挡，不要求她朝着它。
- `remembered` 是短期记忆里「刚才见过、现在不在了」的活物（死了或走远了），最多 5 条：
  死掉的带 `killed_by` = `maid` / `owner` / `other` 与凶手实体 id `killed_by_type`（环境致死
  两者为空）。还活着、此刻仍在她身边的不会列在这里——它们本来就在 `nearby` 里。只在 `query`
  档给；一条记忆被用来解释一次「打不了」之后就会删掉（见 `maid_attack` 的失败原因）。
- `nearby` 按距离升序、最多 3 条，超出时 `truncated` 为 `true`；不含主人与她自己。
  看得见的按档位半径（`query` 32 格、`auto` 工作范围）保留，看不见的只在 `heardRadius`
  （默认 6 格）内保留——地表上走着、脚底下洞里十几格外的僵尸不该算她注意到的东西。
  看得见与否只用遮挡判定，不要求她面朝它。
- `backpack` 是她背包里能交出去的东西（同类合并、最多 12 种，`{item,count}`），给
  「把那个给我」这类指令一个能用的 id 清单；只列背包，不列她手上正在用的工具，也不列
  头饰栏。老模组不返回这个字段，插件不要据此说「背包是空的」。
- `headwear` 是头饰栏（背包最后一格，顶在头上）里的东西，没有就不给这个字段；它与
  `backpack` 互不重复。
- `vehicle` 是她正坐着的载具实体 id（船、矿车、马这类），空串表示没坐；坐着的时候
  `standing_on` 的那一格不是「地」，`ground_change` 也不会报。
- `structure` 是她所在结构的注册表 id（村庄、矿井、要塞这类），空串表示不在结构里。
- `nearby[].name` 只有玩家才有（其余活物的显示名在专用服务端上是翻译键），另一个玩家
  在游戏里只叫得出这个名字，`maid_attack` 也照这个名字点名。
- `blocks` 是附近**能挖的方块种类**，按最近一块的距离升序、去重、最多 12 种；不列空气、
  液体、没有碰撞体积的东西（草、花、火把）和挖不动的方块。玩家准星指着某处时以落点为中心，
  否则以她为中心；范围与 `maid_mine` 找方块时同口径（水平 24 格、竖直 8 格），所以模型可以
  直接拿这里的 id 去下挖掘指令。
- `block_sight` 与 `blocks` 一一对应，`seen` 取 `now`（此刻看见）或 `before`（刚才看见、
  现在不在视野里，来自半分钟的空间记忆）。老模组没有这个字段，插件把整份 `blocks` 当作
  此刻看见；老模组也不返回 `blocks`，插件不要据此说「附近没有」。
- `relations` 与 `blocks` 一一对应，给这一种方块最近那一块的关系：`dx`/`dy`/`dz` 相对她
  脚下那一格，`facing` 同八向方位，`same_layer` 是 `dy=0`，`distance` 是水平距离（格）。
  `blockers` 是视线中间隔着的实心方块数，`blocker` 是第一块是什么；`seen=now` 时必然是 0。
  `walk_steps` 只在八格内做一次有界估算，算不出来是 -1。老模组没有这个字段，插件退回
  只念 `blocks` 清单。
- `clusters` 是同种方块按六面相连数出来的团，只收 `seen=now` 且至少两块的；位置取团里
  最近那一块，字段与 `relations` 同口径。`cells` 数到 64 就停，`truncated` 表示实际更多。
  老模组没有这个字段。
- `map` 只在 `detail=map` 时给。`text` 是她脚下这一层的俯视图：近处一格一单元、远处两格
  合并，`@` 是她、`.` 可走、`#` 挡住、`~` 水、`!` 岩浆与火、`?` 没加载。图不懂高差，
  高差由 `relations.dy` 说；只在玩家问路时给，别当成每轮都要的上下文。
- `owner.same_dimension` 为假时 `distance` 是 -1——插件据此说「玩家不在你这个世界里」，
  而不是编一个距离。
- 女仆不在已加载的区块里、或服务端没就绪时，回统一 `error` 帧。

**`maid_look_around` 是本模组的扩展帧**，社区那套桥接不一定认。老模组收到它会回
`{"type":"error","data":{"message":"不支持的消息类型：maid_look_around"}}`，插件侧要把
这一种如实转达（而不是当成「游戏拒绝了这次动作」）。

## 已知边界

- 女仆走远到区块卸载后查不到，`send_chat` 与 `maid_action` 都会返回「找不到女仆」。车万女仆的
  实体只在已加载区块里拿得到，这是它本身的限制。
- **头顶三档只看她脚下这一层**：四格以内、中间不隔着挡路方块（墙、关着的门）的那些列里，
  还有一列露天就算半遮（树荫、屋檐、洞口），一列都没有才算屋内（房间、洞穴）。开着的门会把
  门外那片露天算进来，石屋与山洞光凭方块分不出，只回头顶那块方块的 id；上下坡不计。
  头顶十六格以上才封着时档位记 `high`，不报具体格数。
- **物品这一层依赖她自己的背包与拾物设置**：没有背包就装不下东西（界面里也放不进去），
  拾物模式没开就不会捡地上的物品。玩家在界面里交接没有事件可挂，只能前后比一次；
  主人就在 8 格内时 `item_received` 带 `from_owner=true`（界面必须贴身才打得开），
  否则只报「身上多了」，不替别人认领。看的是她的背包、双手、盔甲栏与饰品栏四处，
  总数没变就说明只是在她身上挪了个位置（自己吃饭、换工具），不算收到东西；总数涨了才按
  涨的那一处标 `slot`，插件据此说成「塞进手里」「穿在身上」「挂到饰品栏」「放进背包」。
  她自己破坏方块之后的一小段（挖方块，或车万女仆的农场、甘蔗、清雪这类干活）不报：
  那些掉落是车万女仆直接塞进她背包的，不走拾取事件，主人碰巧站在旁边时会被认成
  「主人放进去的」。这条记在 `EntityMaid.dropResourcesToMaidInv` 上，干活与挖矿都涵盖。
  背包差分把双手一起算进来——她自己吃饭时车万女仆会把整摞食物挪到手上，
  只看背包会把放回来的那摞当成「刚收到」。玩家喂的那口同理：食物先摆在她手上、吃完才
  推 `item_fed`，这一段里模组会预先把它记进差分账，不会说成「刚收到」。带汤带碗的食物
  吃完剩下的容器（`usingConvertsTo`，蘑菇煲的碗这类）由车万女仆自己塞进她背包，同样
  预先把这一份记进差分账，并作为 `leftover` 跟着 `item_fed` / `maid_ate` 一起报——
  否则主人站在旁边时，那只碗会被说成「主人往背包里放了碗」。
  反过来少了也一样：主人从她背包里拿走东西原来完全没有感知，她会当成自己弄丢了。现在
  前后比出来的减量走 `item_taken`，但只在**主人就在 8 格内、她自己又没在干活**时才报——
  她干活时背包本来就会少东西（农场播种吃种子、拉弓吃箭），那段说不清是被拿走还是用掉了，
  不去猜。她自己吃掉的、`maid_give_item` 交出去的、没吃成掉回地上的、`maid_place` 放下去的
  那一块，调用方会先记进 `ItemReporter` 的账再扣掉，不会被说成「主人拿走了」；减量按少掉的
  那一处标 `slot`，插件说成「从背包里拿走了」「把手里的拿走了」「把身上的脱了下来」「把戴着的取了下来」。
  看的位置里还包括车万女仆那格隐藏的临时存放：她自己吃饭、喝水时会把手里那件暂收进去，
  吃完再放回手上；不算进来的话，那一顿饭的时间里她的东西就像「少了一件」。
- 单人游戏按 Esc 暂停时游戏 tick 停止，请求会超时。插件那边会提示「游戏可能处于暂停或
  未响应状态」，不是卡死。反过来，动作的计时走的是游戏 tick，暂停期间**不算超时**。
- **居家模式（禁足）会直接拒掉走不到的目标**：车万女仆每个 tick 都会清掉落在活动范围之外的
  寻路目标，所以受理时就预检，超出范围直接回 `out_of_restriction`。
- **玩家在女仆界面里手动改工作模式不会通知她**：那条路没有事件可挂，`maid_task_change`
  只覆盖这个模组自己切的那一次。
- **攻击是模组自己驱动的**：每 tick 追、贴身、按攻击速度出手，不经过车万女仆的攻击 AI，
  所以她的工作模式与作息都不影响打不打得动；但她也追不出 16 格、追不进别的维度。
- **挖掘同样是自己驱动的**：够得着就挖；够不着时先走过去，挡着就把挡路的挖穿（最多 24 块）。
  目标在岩浆里、被保护、挖不动都会如实失败。掉落物走她的背包（车万女仆直接把掉落塞进库存），没有背包时掉在地上
  （她自己的拾物设置决定捡不捡）。挖着和刚挖完的几秒里，背包增量不再触发
  `item_received`——那些都是她自己挖下来的，结果由 `maid_mine_done` 交代。
  连挖时第一块看她的位置，之后按「离刚挖掉的那块最近」找（树干上方那几块就是窗口跟着
  上移）；水平找 32 格。落脚点按离目标最近挑、竖直往下多看 6 格，并且**走到落脚点才动手**——
  原来一进五格交互距离就开挖，看起来像是在离树很远的地方挥镐。
  整团拆与区域拆同样走这套走位与挖穿：一次最多 64 块、每条边最多 16 格；拆到一半被打断
  （走不到、超时、被抱走）就停下，剩余与卡住的格子由 `maid_mine_done` 交代，不会自己接着拆。
  区域里的方块按离她远近挑，受保护或缺工具的方块在受理时先剔掉。
- **有活的时候不会被拽回来**：车万女仆的 `MaidFollowOwnerTask` 在主人离她超过「活动半径 + 2」时
  会把正在干活的女仆直接传送回主人身边，顺手清掉寻路目标。桥接用一个很小的服务端 mixin：
  她手上有活（攻击、挖方块、走过去/躺床/探路，或非空闲工作模式）**且主人已经超出原版回头线**
  时，取消这个行为的启动；主人靠近时照常跟随，活干完或切回空闲立刻恢复。探路期间同理——
  不然她刚一走到主人前面就会被传送回去。
  攻击类的「有活」看**此刻有没有目标**：追打的时候可以离得很远，目标一没就恢复默认跟随距离
  （不然她追出去很远之后会留在原地不回来）。
  **不动她的活动半径**，所以拾物、找床、找餐桌、跟随的距离全部与车万女仆原版一致。
- **被抱起来的判定有两条路**：主路径是 Carry On 自己的 `EntityPickupEvent` 与
  `EntityJoinLevelEvent`；事件挂不上（版本对不上）时退回每 tick 翻玩家 carry NBT 的 UUID。
  被抱着期间她不在世界里，气泡、动作与「看一眼周围」都用不了。
- **躺下按的是「睡位 + 睡姿」**：地上的躺姿也在脚下按一个睡位，YSM 的闭眼与躺姿、车万女仆的
  「别乱走」都只认 `isSleeping()`；原版「睡位不是床就起来」与车万女仆的日程清理由
  `LivingEntityStopSleepingMixin` 在动作期间拦掉。她自己起来只有挨打、泡进液体、身下没地、
  被抱走、床没了这几种，或者被新指令顶掉。地上的躺姿沿用原版睡姿的渲染：没有床时方向不会
  跟着她的朝向走（原版那条兜底在 1.21.1 走不到），要改得碰客户端渲染；躺姿碰撞箱由
  `LivingEntityLyingDimensionsMixin` 放宽到 0.9 宽、0.4 高，右键与搬运不必只对着脚。
- **床上的睡位按床头算**：车万女仆每 tick 把人吸到睡位那一格的中心，渲染又按睡位那格床的
  朝向把身子往床尾铺开一格半。睡位落在床尾那半张，头就离开枕头、身子往床外错出去一格，
  所以床尾会顺着 `FACING`（两套床都是「床尾指向床头」）翻到床头；人不在床头那格就摆回
  床头中心，踩在任意一半上都会被对齐。昏迷后被搬上床同理，恢复计时也按床头压睡位。
  床上休息每 10 秒回一滴血，地上的躺姿不回。
- **讨食走的是车万女仆自己的任务**：模组只把插件发来的食物接进她的诱惑物（`getTemptationItem`），
  所以「主人拿在主手上、她看得见、六格以内」这些条件与原版蛋糕完全一致；她在干活、打架或
  睡着时不讨，`favorite_food` 也不会推。食物清单是插件在连上时与玩家每次开口前发来的
  （`set_favorite_food`），模组不设自己的配置项；改完设置下一次在游戏里说话时生效。
  填完整的物品 id 就按那个命名空间认，只写后半段时原版优先、其余模组同名的也算；
  带负面效果的食物（毒马铃薯、河豚这类）不算，认不出食物组件的（有些模组食物不带原版的
  食物组件）照常算。
- **探路只认主人此刻的朝向**：落点按朝向 8 格算，不查主人的视线与来路，所以她把路带进
  死胡同、带到悬崖下都是可能的；落点站不下人时退回跟住主人。居家模式下只在她自己的活动
  范围里找点，范围边缘会停住，直到主人走出 24 格判定走散。
- 跟随模式、玩家开着女仆 GUI 时，车万女仆自己的逻辑会和我们的目标抢：表现为走不动，
  最后按 `unreachable` / `timeout` 如实认输，不会一直卡着。
- 床边那一格常常走不进去，所以「贴近了但不再靠近」会当成到了；判定阈值与寻路口径一致
  （玩家 3 格、床 2 格，曼哈顿距离）。
- 端口开在本机，任何本地进程都能连上来让女仆说一句话、做上表里的这几个动作。动作参数全部
  过白名单，接口层面没有坐标入口，也没有执行指令的能力。
- **她看不见的东西不该知道**：身边这一层只用她自己的位置、视线、光照与周围的实体，
  不读主人视角的信息；站在她背后但没遮挡的东西算她知道（背后有人走近本来就该有感觉），
  被墙挡住的才算不知道。
- 身边这一层的**第一帧只建基线、不出任何事件**：刚接上、换了监控对象、女仆离开已加载
  区块再回来，都不会因为「基线是空的」而喷一屏变化。女仆离开再回来时全局那层会重推一条
  `state_snapshot`，那是设计好的「我丢线了」信号。
- 事件密集时插件的信号并发上限（全局 4 个槽，与逐句同步共用）可能丢掉多余的帧——这是
  宿主侧的已知限制。身边这一层靠每类各自的冷静期把每帧压到 0~2 条来规避。
- 协议里**没有坐标**：只有档位（`close` / `near` / `far` / `gone`）、类别与四舍五入过一位
  的距离。查询接口同样不收坐标。
