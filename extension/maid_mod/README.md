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
- 角色的意图 → 女仆真的走过去、坐下、站起来、躺到床上，换一种工作模式（种菜、钓鱼、打怪），
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
| `action/` | 会改变世界或女仆状态的行为。`core` 是通用动作框架（走到/坐下/躺下），`attack`、`mine`、`task`、`give`、`feed` 各自独立；`LongTaskGuard` 判断「她手上有没有活」。 |
| `bridge/` | 本机 WebSocket、信封、路由、协议常量。 |
| `chat/` | 游戏内聊天双向：玩家打字推给 LingChat，角色回复变成气泡。 |
| `perception/` | 一切「观察世界」。`world` 全局采样、`nearby` 身边采样、`query` 按需查询、`event` 受伤/死亡/成就、`item` 物品与背包、`carry` 抱放；`EventFrames`、`PerceptionProfiles`、`PerceptionSampler` 是这几层共用的帧外形、半径与采样门控。 |
| `maid/` | 女仆身份、查询、监控目标、状态与背包只读清单。 |
| `integration/` | 可选模组适配；目前只有 `carry`（Carry On）。 |
| `mixin/` | NeoForge mixin，包名被 `lingchat_maid_bridge.mixins.json` 固定，不要随意移动。 |

判定与读世界分开：`*Diff`、`*Rules`、`BackpackDiff` 是纯逻辑，能脱开 Minecraft 跑单测；`*Reader`、`MaidActuator`、`MineTargets` 才碰游戏。

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
| 插件 → 模组 | `set_monitored_maid` | 告诉模组盯哪只女仆。**不发这条，玩家说话不会被推送** |
| 插件 → 模组 | `send_chat` | 让女仆说一句话 |
| 插件 → 模组 | `maid_action` | 让女仆做一个动作（见下） |
| 插件 → 模组 | `maid_set_task` | 给她换一个工作模式（见下） |
| 插件 → 模组 | `maid_attack` | 让她去打指定的那个活物（见下） |
| 插件 → 模组 | `maid_mine` | 让她去挖方块（见下） |
| 插件 → 模组 | `maid_give_item` | 把她背包里的东西交给玩家（见下） |
| 插件 → 模组 | `maid_look_around` | 取一份她身边的详细快照（只读，见下） |
| 模组 → 插件 | `maid_action_result` | 动作的受理回执 |
| 模组 → 插件 | `maid_set_task_result` | 换工作模式的回执 |
| 模组 → 插件 | `maid_attack_result` | 攻击指令的回执 |
| 模组 → 插件 | `maid_mine_result` | 挖掘指令的回执 |
| 模组 → 插件 | `maid_give_item_result` | 交物品的回执 |
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

### `maid_action`：动作

```json
{"type":"maid_action","data":{"maid_id":"...","action":"go_to|sit|stand|lie_down|look_at","target":"player|bed"}}
```

`action` 与 `target` 都过白名单；认不出的 `target` 退回默认（走 = 玩家、躺 = 最近的床），
认不出的 `action` 直接拒绝。**接口层面没有接受坐标的入口**。

`look_at` 保留在协议里只为兼容：插件已经不再把它注册成工具，她说话时会自动看向主人
（每来一句气泡转一次头，不占动作槽、不推事件）。

| 动作 | 目标 | 落地 | 受理即完成？ |
| --- | --- | --- | --- |
| `go_to` | `player`（默认） | 每 tick 写 `WALK_TARGET`，由原版寻路走过去 | 否，到位后推 `action_done` |
| `go_to` | `bed` | 找最近的空床（原版床与车万女仆的女仆床都算），走到床头那格 | 否，到位后推 `action_done` |
| `sit` | — | 坐下；坐姿下她不会自己走动 | 是 |
| `stand` | — | 站起来；正睡着会被叫醒，坐在坐垫/椅子/船上会下来 | 是 |
| `lie_down` | `bed`（唯一取值） | 先走到床边，再按她自己的睡床逻辑睡下 | 否，睡着或被叫醒后推 `action_done` |
| `look_at` | `player` | 转头看向玩家，保持约 2 秒 | 是（到时静默收尾，不推事件） |

回执：

```json
{"type":"maid_action_result","data":{"accepted":true,"async":true,"action":"go_to","target":"player",
 "replaced_action":"","stood_up":false,"reason_code":"","reason":""}}
```

- `accepted=false` 表示**受理时就能判定失败**：附近没床、床都被占了、玩家不在同一维度、
  目标超出居家活动范围、女仆在睡觉 / 被骑 / 被拴 / 找不到。这时 `reason` 是中文原因，
  `reason_code` 是给程序看的短码。
- `async=true` 表示结果稍后由 `action_done` 给出（只有 `go_to` 与 `lie_down` 是这种）。
- `stood_up=true` 表示为了这次动作先让她站起来了（坐着时她根本不会迈步）。
- `replaced_action` 是被这次动作顶掉的那个动作名；同一时刻**只允许一个动作**在跑，
  被顶掉的那次会收到一条 `status=cancelled` 的 `action_done`。

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
{"type":"maid_attack","data":{"maid_id":"...","target":"minecraft:zombie","style":"fight"}}
```

`target` 四种写法：`owner`（她主人）、`hostile`（最近的敌对生物）、`animal`（最近的动物）、
或一个实体注册表 id（打最近的那一只）。插件负责把「僵尸」「zombie」翻成 id，写全命名空间
的模组实体也认。

`style` 两种力度：`fight`（默认，不给也是它）追上去一直打到对方倒下；`light` 只轻轻打
一下就收手——那一下的伤害固定 1 点（半点血，穿甲更少），也不会替她换武器，用来逗她抬手。

- **不走车万女仆的 `canAttack` 白名单**：主人自己、村民、动物、别的玩家都能点名。这是这个
  工具存在的意义，该打谁由玩家决定。仍会排除盔甲架与她自己的实体。
- **自己驱动，不依赖她的作息**：车万女仆的攻击行为只在「WORK 日程 + 攻击模式」下才跑，
  而且目标有效范围按居家/跟随模式算，稍远就清掉。所以这里不复用它那套，而是在
  `MaidAttackController` 里每 tick 推进：写 `WALK_TARGET` 追过去、贴身之后 `doHurtTarget`，
  出手间隔按她的攻击速度算。玩家半夜喊一句也能打。
- **顺手换武器**：背包里有带攻击力的东西就换到手上（和车万女仆自己的攻击模式同一口径），
  没有就空手打，回执里 `weapon=false`，插件会说明伤害很低。
- **目标范围**：找目标看 32 格（用来区分「没有」与「太远」）。居家模式下目标要在她的活动
  范围里，否则当场拒绝（`target_too_far`）；跟随模式不再限制「主人到目标」的距离——追打期间
  跟随行为的传送被 `LongTaskGuard` 拦住了（见「已知边界」），打完收工后跟随恢复。
- **收工**：目标死了、被打断（新动作、换工作模式）或追满 30 秒，循环就停。她打死目标时
  由死亡旁白交代「被你杀了」，这里不重复。
- 回执（`maid_attack_result`）：`accepted`、`weapon`、`style`，以及
  `target{id,type,name,distance,owner}`。

### `maid_mine`：挖掉方块

```json
{"type":"maid_mine","data":{"maid_id":"...","target":"looked","count":1}}
```

`target` 两种写法：`looked`（默认，玩家准星指着的方向）或一个方块注册表 id
（`minecraft:stone`，插件负责把「石头」这类中文翻成 id）。也接受方块标签
（`#minecraft:logs`，插件把「木头」这类泛称翻成它）。`count` 是这一次挖几块，
默认 1，上限 64。

- **玩家指哪片就在哪片挖**：准星射线放宽到 32 格，落点当作区域中心（不必凑到交互距离内）。
  写了方块名时，先在这一带 8 格内找这种方块，找到就只在这一带挖；`looked` 则先挖落点那块，
  再接着在这一带找同种方块。准星没指着、或者指着的那片没有目标时，才退回以她为中心找
  （24 格）。
- **按族找**：`#minecraft:logs` 这类标签按方块标签匹配（哪片林子都能砍），普通矿石 id
  自动带上深层变体（`iron_ore` 也认 `deepslate_iron_ore`），不会因为地下只剩深层矿而
  报「附近没有」。

- **自己驱动，不占任务槽**：和攻击一样不走车万女仆的任务/AI，`MaidMineController` 每 tick
  推进——找一块站得下、够得着、看得见、寻路走得到的落脚点，走过去，然后朝方块挖。
  她此刻在种地、钓鱼还是闲着都不影响。
- **按原版速度挖**：进度按方块硬度与手上工具算，正确工具是 `速度 / 硬度 / 30`、用错工具或
  徒手是 `/100`，只有硬度为负的方块（基岩）挖不动。裂纹按 `destroyBlockProgress` 发给
  客户端，玩家看得见进度，不会瞬破。
- **自己换工具**：从背包里挑挖这一块最快、且能挖出掉落的工具换到手上（木镐不会顶掉钻石镐，
  剑也不会被拿去挖石头）。方块要求正确工具而她没有时当场拒绝（`wrong_tool`）；不要求工具
  的方块徒手也挖，只是慢。每挖穿一块扣一点耐久，挖断了就换背包里下一把。
- **掉落进她的背包**：走车万女仆自己的 `destroyBlock`（带 `canEntityDestroy` 与
  NeoForge 的实体破坏事件），受保护的方块不挖；掉落物直接进背包，装不下才掉在地上。
- **不受作息与工作模式限制**：玩家半夜喊一句也能挖。
- **收工**：挖够、附近没有同类方块、走不到、超时（单块 60 秒）都会推一条
  `maid_mine_done`（见下），不会留下「她到底挖完没有」的悬案。

回执（`maid_mine_result`）：

```json
{"data":{"accepted":true,"async":true,"looked":false,"count":3,"block":"minecraft:stone",
 "name":"Stone","requires_tool":true,"has_tool":true,"tool":"minecraft:iron_pickaxe",
 "replaced_action":"","reason_code":"","reason":""}}
```

- `accepted=false` 是受理时就能判死的失败：没有这种方块、指着的那片和她附近都没有能挖到的、
  方块挖不动、受保护、她走不到跟前、没有能挖它的工具、玩家不在这里。`reason` 是中文原因。
- `async=true`：受理只代表开始挖了。挖穿 / 挖够 / 走不到由 `maid_mine_done` 事件交代，
  字段是 `status`（`done` / `partial` / `failed`）、`mined`、`wanted`、`block`、
  `reason_code`、`reason`。**插件在那之前不能说她挖到了。**
- `has_tool=false` 表示她手上和背包里都没有趁手的工具，只能徒手挖（会慢一些）。
- 与攻击、动作、换工作模式互斥：新的挖掘令会顶掉它们，它们也会顶掉挖掘。

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

### 喂她吃饭：潜行 + 右键食物

车万女仆自己只认潜行喂金苹果 / 附魔金苹果，普通食物右击只会打开她的界面。本模组补上另一半：

- **潜行 + 右键任意食物**：吃掉手里这一个、按营养值回一点血、播进食音效，并推一条
  `item_fed`。这一下在 `InteractMaidEvent` 的 `HIGHEST` 就被取消，切坐姿（车万女仆的
  `LOWEST`）与别的附属模组的右键逻辑都收不到——拿着食物潜行出手只会喂饭，不会顺带坐下。
- **金苹果 / 附魔金苹果**：仍走车万女仆自己的 `ApplyGoldenAppleEvent`（成就照旧），本模组
  只在 `HIGHEST` 留一份副本，等她**真的吃完**（`MaidAfterEatEvent`）才补记一条；交互被
  别的模组取消、她没吃到，就不会误报。
- **空手与搬运时不碰**：空手右键在 Carry On 那边是抱起/放下；玩家手里抱着任何东西、或者
  她自己被抱起来（成为玩家的乘客）时也不喂、不取消交互，免得和搬运模组抢同一个手势。
- **创造的玩家**：食物不真的扣，效果与回血照旧。
- 她睡着时右键不喂；普通右键（不潜行）不受影响，仍然打开界面。
- **她自己吃**：车万女仆的进食系统（受伤后从背包找东西吃）完成时，模组推一条
  `maid_ate`，与玩家喂食分开说。判定用 `LivingEntityUseItemEvent.Finish`，因为
  `MaidAfterEatEvent` 只给吃完剩下的那份，普通食物那里是空的，认不出吃的是什么。

### `event`：世界的事件

只在**变化显著**或**状态未初始化**时推，不是每次采样都推。字段是原始的游戏 id
与数值，中文措辞由插件那侧决定（游戏语义留在 Java，自然语言理解留在 Python）。

| `event_type` | 触发 | 主要字段 |
| --- | --- | --- |
| `state_snapshot` | 刚接上 / 刚绑定女仆（含重连） | `dimension` `biome` `sky` `phase` `raining` `thundering` 双方血量 |
| `biome_change` | 换了生物群系（30 秒内只报一次） | `from_biome` `biome` |
| `dimension_change` | 换了维度（同一帧不再报群系） | `from_dimension` `to_dimension` |
| `time_phase_change` | 昼夜翻转（只推翻转那一次） | `phase` = `day` / `night` |
| `weather_change` | 开始 / 停止下雨、打雷 | `raining` `thundering` |
| `low_health` | 玩家或女仆血量跌破阈值（边沿触发） | `who` `name` `health` `max_health` |
| `maid_hurt` | 女仆挨打（一小段连续挨打合成一条，两次上报至少隔 5 秒） | `damage` `count` `attacker` `damage_type` `health` `max_health` |
| `creature_death` | 附近（`noticeRadius` 内）有生物死亡（10 秒内只报第一条） | `type` `category` `distance` `attacker` `attacker_type` `damage_type` `victim_owner` `attacker_owner` `attacker_maid` |
| `player_advancement` | 主人拿到了成就（原版「进度」；配方解锁这类没有展示信息的跳过） | `advancement` `title` `frame` = `task` / `goal` / `challenge` |
| `maid_carried` | 她被 Carry On 抱了起来（没装那个模组就不会出现） | — |
| `maid_dropped` | 她被放了下来 | — |
| `action_done` | 动作收尾：成功、失败或被新动作顶掉 | `action` `target` `status` `reason_code` `reason` `elapsed_ticks` `distance` |
| `player_proximity_change` | 与主人的远近跨档（回滞，30 秒内只报一次） | `from_proximity` `proximity` = `close` / `near` / `far` / `gone`，`distance` |
| `player_sight_change` | 主人进出视线（要连续两次采样一致才认） | `visible` |
| `threat_near` | 附近**从无到有**敌对生物，或最近那只由远转近 | `type` `distance` `direction` `visible` |
| `living_near` | 身边来了别的活物（主人以外的生物、别的玩家、别的女仆） | `type` `category` `distance` `direction` |
| `shelter_change` | 从露天走进遮蔽处，或反过来 | `sheltered` |
| `ground_change` | 脚下换了种值得说的地（水、岩浆、火、雪、沙、耕地、冰） | `block` `kind` |
| `item_gift` | 主人把手里的物品丢在她身边（一小段连续投掷合成一条） | `items[{item,count}]` `distance` |
| `item_pickup` | 她捡起地上的物品 | `item` `count` `from_owner` |
| `item_received` | 她的背包里多出物品（多半是玩家在女仆界面里放的） | `items[{item,count}]` `from_owner` |
| `item_fed` | 玩家喂她吃了东西（连喂几口合成一条） | `items[{item,count}]` |
| `maid_ate` | 她自己从背包里找东西吃了（受伤后进食；停口 6 秒才算一顿，合并成一条） | `items[{item,count}]` |

身边这一层 `threat_near` 与踩进岩浆 / 火里的 `ground_change` 算紧急，插件会当场请角色开口；
其余（含 `shelter_change`、普通地面变化）走「攒够几条」那套。
`creature_death` 里死的是她主人时也当场开口。
物品那几条另有一套：收到玩家给的东西、被喂饭当场开口，玩家丢下的东西被她捡起来也当场
开口；她自己捡到杂物、玩家只是丢在她脚边、她自己吃了东西，都只记旁白。

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

写请求响应时有两件事必须守住：响应要把 `request_id` 原样带回，而推送帧**绝不能**
带 `request_id`——LingChat 宿主按 `request_id` 认领响应，误带会让推送帧被当成某个
请求的应答吞掉。

### `maid_look_around`：看一眼她周围

```json
{"type":"maid_look_around","data":{"maid_id":"...","profile":"query"}}
```

- `profile` 缺省或 `query`：玩家明确问「你周围有什么」，看得见的目标放宽到 32 格，并附
  `blocks` / `backpack`。`profile=auto`：玩家开口时顺手带的那份近处快照，看得见按工作范围、
  看不见只听 `heardRadius` 以内，不扫方块与背包（省掉每次开口的方块读取）。

回帧（`maid_look_around_result`）：

```json
{"data":{
  "maid_id":"...",
  "dimension":"minecraft:overworld",
  "looking_at":{"kind":"block","id":"minecraft:oak_door","distance":4.3},
  "standing_on":{"id":"minecraft:farmland","footing":"farmland"},
  "sheltered":false, "has_sky":true, "light":12,
  "owner":{"name":"NoiQing_Ling","distance":3.4,"visible":true,"same_dimension":true},
  "nearby":[{"type":"minecraft:zombie","category":"hostile","distance":6.8,"direction":"left","visible":false}],
  "blocks":["minecraft:spruce_log","minecraft:dirt","minecraft:stone"],
  "backpack":[{"item":"minecraft:rose_bush","count":2},{"item":"minecraft:iron_pickaxe","count":1}],
  "truncated":false
}}
```

- 一律只给注册表 id：专用服务端上 `getName()` 只有英文（甚至裸翻译键），中文只在客户端
  按玩家语言选得出来，所以中文名全在插件那侧定。
- `looking_at.kind` 取 `block` / `entity` / `none`；方块用 OUTLINE 射线，草丛、火把、
  告示牌这些「看得见但没碰撞」的也算挡着（与视线判定的口径刻意不同：玻璃挡得住这条
  射线，挡不住视线）。
- `direction` 是相对她**身体**朝向的八向：`front` `front_right` `right` `back_right`
  `back` `back_left` `left` `front_left`。她不一定是面朝哪就看哪，而「她周围有什么」
  说的是她站着的方位；只有 `looking_at` 那条射线用头部朝向。
- `visible` 是「她看不看得见」，判据是无遮挡，不要求她朝着它。
- `nearby` 按距离升序、最多 3 条，超出时 `truncated` 为 `true`；不含主人与她自己。
  看得见的按档位半径（`query` 32 格、`auto` 工作范围）保留，看不见的只在 `heardRadius`
  （默认 6 格）内保留——地表上走着、脚底下洞里十几格外的僵尸不该算她注意到的东西。
  看得见与否只用遮挡判定，不要求她面朝它。
- `backpack` 是她背包里能交出去的东西（同类合并、最多 12 种，`{item,count}`），给
  「把那个给我」这类指令一个能用的 id 清单；只列背包，不列她手上正在用的工具。老模组
  不返回这个字段，插件不要据此说「背包是空的」。
- `blocks` 是附近**能挖的方块种类**，按最近一块的距离升序、去重、最多 12 种；不列空气、
  液体、没有碰撞体积的东西（草、花、火把）和挖不动的方块。玩家准星指着某处时以落点为中心，
  否则以她为中心；范围与 `maid_mine` 找方块时同口径（水平 24 格、竖直 8 格），所以模型可以
  直接拿这里的 id 去下挖掘指令。老模组不返回这个字段，插件不要据此说「附近没有」。
- `owner.same_dimension` 为假时 `distance` 是 -1——插件据此说「玩家不在你这个世界里」，
  而不是编一个距离。
- 女仆不在已加载的区块里、或服务端没就绪时，回统一 `error` 帧。

**`maid_look_around` 是本模组的扩展帧**，社区那套桥接不一定认。老模组收到它会回
`{"type":"error","data":{"message":"不支持的消息类型：maid_look_around"}}`，插件侧要把
这一种如实转达（而不是当成「游戏拒绝了这次动作」）。

## 已知边界

- 女仆走远到区块卸载后查不到，`send_chat` 与 `maid_action` 都会返回「找不到女仆」。车万女仆的
  实体只在已加载区块里拿得到，这是它本身的限制。
- **物品这一层依赖她自己的背包与拾物设置**：没有背包就装不下东西（界面里也放不进去），
  拾物模式没开就不会捡地上的物品。玩家在界面里交接没有事件可挂，只能前后比一次；
  主人就在 8 格内时 `item_received` 带 `from_owner=true`（界面必须贴身才打得开），
  否则只报「背包里变多了」，不替别人认领。背包差分把双手一起算进来——她自己吃饭时
  车万女仆会把整摞食物挪到手上，只看背包会把放回来的那摞当成「刚收到」。
- 单人游戏按 Esc 暂停时游戏 tick 停止，请求会超时。插件那边会提示「游戏可能处于暂停或
  未响应状态」，不是卡死。反过来，动作的计时走的是游戏 tick，暂停期间**不算超时**。
- **居家模式（禁足）会直接拒掉走不到的目标**：车万女仆每个 tick 都会清掉落在活动范围之外的
  寻路目标，所以受理时就预检，超出范围直接回 `out_of_restriction`。
- **玩家在女仆界面里手动改工作模式不会通知她**：那条路没有事件可挂，`maid_task_change`
  只覆盖这个模组自己切的那一次。
- **攻击是模组自己驱动的**：每 tick 追、贴身、按攻击速度出手，不经过车万女仆的攻击 AI，
  所以她的工作模式与作息都不影响打不打得动；但她也追不出 16 格、追不进别的维度。
- **挖掘同样是自己驱动的**：只挖够得着、看得见、走得到跟前的方块，不会为了挖一块矿向下
  开隧道（那是「挖矿工作模式」的事，不在这一期）。找不到落脚点、被墙挡住、目标在岩浆里
  都会如实失败。掉落物走她的背包，没有背包时掉在地上（她自己的拾物设置决定捡不捡）。
- **有活的时候不会被拽回来**：车万女仆的 `MaidFollowOwnerTask` 在主人离她超过「活动半径 + 2」时
  会把正在干活的女仆直接传送回主人身边，顺手清掉寻路目标。桥接用一个很小的服务端 mixin：
  她手上有活（攻击、挖方块、走过去/躺床，或非空闲工作模式）**且主人已经超出原版回头线**
  时，取消这个行为的启动；主人靠近时照常跟随，活干完或切回空闲立刻恢复。
  **不动她的活动半径**，所以拾物、找床、找餐桌、跟随的距离全部与车万女仆原版一致。
- **被抱起来的判定有两条路**：主路径是 Carry On 自己的 `EntityPickupEvent` 与
  `EntityJoinLevelEvent`；事件挂不上（版本对不上）时退回每 tick 翻玩家 carry NBT 的 UUID。
  被抱着期间她不在世界里，气泡、动作与「看一眼周围」都用不了。
- **白天的「躺下」会被她自己叫醒**：车万女仆按日程决定活动，不在睡觉时段时她躺下就会起身。
  这时动作如实报 `not_sleep_time`，不是 bug；档案调成「全天工作」的话更是永远睡不着。
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
