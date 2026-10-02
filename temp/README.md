# screen_cast 发送端（PC 端工具）

配合 HoloCubic 上的 **无线投屏（screen_cast）** 应用使用：抓取电脑屏幕区域，**保持源比例**缩放后居中贴到 **320×240** 画布（16:9 画面为 320×180 居中、上下留黑边，不会拉伸变形），编码为基线 JPEG，以 `multipart/x-mixed-replace` 的 MJPEG 流在局域网内推送，设备拉流并实时显示。

## 安装

需要 Python 3.8+：

```bash
python -m pip install mss pillow
```

可选（`--url` headless 模式）：

```bash
python -m pip install playwright
python -m playwright install chromium
```

## 运行

```bash
# 抓主屏并推流（默认端口 8080）
python cast_sender.py

# 全息屏颜色补偿（颜色被透光洗淡时用这个，饱和度+对比度一键增强）
python cast_sender.py --vivid

# 指定端口 / 帧率 / JPEG 质量
python cast_sender.py --port 8080 --fps 15 --quality 75

# 只抓屏幕某一块区域
python cast_sender.py --x 200 --y 120 --w 640 --h 480

# 抓指定显示器（0=主屏）
python cast_sender.py --monitor 1

# headless 截取一个网页并推流（需 playwright）
python cast_sender.py --url https://example.com
```

启动后会打印本机局域网 IP 与推流地址，例如：

```
[screen_cast] 本机局域网 IP：192.168.0.23
[screen_cast] MJPEG 流：   http://192.168.0.23:8080/stream
[screen_cast] 内容缩放后 320x180，居中贴到 320x240 黑底画布（上下黑边各 30px）
[screen_cast] 设备 WebUI 如需四周透出：把“内容高度 content_h”设为 180（0=全屏显示黑边）
```

> 设备 `jpg.so` 只解码 **320×240** 的帧，因此本工具始终把内容**居中贴回 320×240 黑底画布**再编码；黑边默认不透明。想在透光/全息屏上让黑边透出，就把设备 WebUI 的 `content_h` 设成上面打印的内容高度 M（16:9 为 `180`）。

## 与设备连接

1. 在电脑上运行 `cast_sender.py`，记下打印的局域网 IP。
2. 打开设备的 WebUI 控制台（HoloCubic 启动器里进入无线投屏的网页控制台，默认 `http://<设备IP>/screen_cast/`）。
3. 传输方式选 **MJPEG over HTTP**，填写：
   - 发送端 IP：`192.168.0.23`
   - 端口：`8080`
   - MJPEG 路径：`/stream`
4. 点“保存并重连”，设备上出现画面即拉流成功（画面一上屏，顶部状态条自动隐藏；只有连接中/离线时才会显示）。
5. 按方向键下在 `bg_opa=0`（透明）与 `255`（不透明）之间切换对比；方向键上切换状态条。要让 16:9 画面四周透出（透明黑边），把设备 WebUI 的 **内容高度 content_h** 设成下面启动日志打印的高度（16:9 为 `180`）；`0` 则全屏显示黑边。

## 浏览器 / ffmpeg 交叉验证

```bash
# 浏览器打开预览页，能看到 MJPEG 画面即说明发送端正常
# 地址：http://<本机IP>:8080/

# ffmpeg 录制验证（可另存为文件）
ffmpeg -i http://192.168.0.23:8080/stream out.avi
```

## 参数一览

| 参数                | 默认       | 说明                                                                                                    |
| ------------------- | ---------- | ------------------------------------------------------------------------------------------------------- |
| `--monitor N`       | `0`        | 显示器编号，0=主屏                                                                                      |
| `--x --y --w --h`   | 整屏       | 抓取区域（同时给 `--w --h` 时生效）                                                                     |
| `--port`            | `8080`     | HTTP 端口                                                                                               |
| `--fps`             | `15`       | 目标帧率（设备实测约 13–14 FPS 为解码上限）                                                             |
| `--quality`         | `85`       | JPEG 质量 1–100（越高越清晰、体积越大）                                                                 |
| `--subsampling`     | `444`      | JPEG 色度子采样：`444`=色彩最准（默认）、`422`/`420`=体积更小但色彩略差、`keep`=跟随源图                |
| `--resample`        | `lanczos`  | 缩放插值（清晰度）：`lanczos` 最清晰（默认）、`bicubic` 较清晰、`bilinear` 柔和、`nearest` 锐利但易锯齿 |
| `--vivid`           | 关         | 颜色增强预设：`saturation=1.4`、`contrast=1.15`（全息/透光屏颜色被洗淡时用）                            |
| `--saturation`      | `1.0`      | 饱和度倍率（`>1` 更鲜艳，建议 `1.3`–`1.5`；`1.0` 不处理）                                               |
| `--contrast`        | `1.0`      | 对比度倍率（`>1` 色彩更深，配合 `--saturation` 用；`1.0` 不处理）                                       |
| `--optimize`        | 关         | 启用 JPEG Huffman 优化（编码略慢、体积更小）                                                            |
| `--url`             | 无         | headless 截取网页推流                                                                                   |
| `--page-w --page-h` | `1280×720` | `--url` 模式的视口                                                                                      |

| `--ws` | 无 | Phase 2 预留：WebSocket 音频/麦克风桥接端口（当前版本未启用） |

画面糊/色彩丢失时：优先把 `--quality` 调到 90+，并保持 `--subsampling 444`（色彩丢失几乎全由 4:2:0 子采样引起）；`--resample lanczos` 已是默认。

## 说明

- 输出固定为 **320×240 基线 JPEG**，与设备 `jpg.so`（ROM TJpgDec）兼容；发送端不输出渐进式 JPEG。
- 设备端拉流解析只认 JPEG 的 SOI/EOI 标记，因此本工具即使将来改为裸 JPEG 流也无需改动设备端。
- 音频播放与麦克风输入（Phase 2）将走 `--ws` 端口开启的 WebSocket 通道，当前版本仅预留端口参数。
