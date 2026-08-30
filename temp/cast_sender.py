#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
screen_cast 发送端工具：抓取电脑屏幕区域，缩放到 320x240 后以 MJPEG 流推给 HoloCubic。

用法示例
--------
  # 抓主屏并推流（默认）
  python cast_sender.py

  # 指定分辨率/质量/帧率
  python cast_sender.py --port 8080 --fps 15 --quality 75

  # 抓某个屏幕区域
  python cast_sender.py --x 200 --y 120 --w 640 --h 480

  # 抓指定显示器（0=主屏）
  python cast_sender.py --monitor 1

  # （可选）headless 截取一个网页并推流，需要 playwright + chromium
  python cast_sender.py --url https://example.com

依赖
----
  python -m pip install mss pillow
  可选：playwright（--url 模式）

浏览器交叉验证：打开 http://<本机IP>:8080/ 应能看到画面；
ffmpeg 验证：  ffmpeg -i http://<本机IP>:8080/stream out.avi
"""
import argparse
import io
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

TARGET_W = 320
TARGET_H = 240
BOUNDARY = b"screen_cast_frame"

# 缩放插值 / JPEG 色度子采样（PIL 常量）
RESAMPLE_MAP = {
    "nearest": "NEAREST", "bilinear": "BILINEAR", "bicubic": "BICUBIC", "lanczos": "LANCZOS",
}
SUBSAMPLING_MAP = {"444": 0, "422": 1, "420": 2}

try:
    import mss
except ImportError:
    mss = None


def lan_ip():
    """尽力获取局域网 IP（UDP 连接法，不发包）。"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(("8.8.8.8", 80))
        return sock.getsockname()[0]
    except Exception:
        return "127.0.0.1"
    finally:
        sock.close()


def make_screen_shot(args):
    """返回一个 shot_fn() -> PIL.Image（RGB）。屏幕捕获。"""
    if mss is None:
        raise SystemExit("缺少依赖 mss：请先执行  python -m pip install mss pillow")

    sct = mss.MSS() if hasattr(mss, "MSS") else mss.mss()
    if args.monitor is not None:
        monitor = sct.monitors[args.monitor + 1] if args.monitor + 1 < len(sct.monitors) else sct.monitors[1]
    else:
        monitor = sct.monitors[1]  # 主屏

    crop = (args.w and args.h and (args.x is not None and args.y is not None))
    box = None
    if crop:
        box = (args.x, args.y, args.x + args.w, args.y + args.h)

    def grab():
        shot = sct.grab(monitor)
        img = Image_frombytes(shot)
        if box:
            img = img.crop(box)
        return img

    return grab


def Image_frombytes(shot):
    from PIL import Image
    return Image.frombytes("RGB", shot.size, shot.bgra, "raw", "BGRX")


def make_url_shot(args):
    """返回一个 shot_fn() -> PIL.Image（RGB）。headless 截取网页，需要 playwright。"""
    from PIL import Image
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        raise SystemExit("--url 模式需要 playwright：python -m pip install playwright && python -m playwright install chromium")

    pw = sync_playwright().start()
    browser = pw.chromium.launch(headless=True)
    page = browser.new_page(viewport={"width": args.page_w, "height": args.page_h})
    page.goto(args.url, wait_until="domcontentloaded")
    print(f"[screen_cast] 已加载页面：{args.url}")

    def grab():
        data = page.screenshot()
        return Image.open(io.BytesIO(data)).convert("RGB")

    return grab


class CaptureThread(threading.Thread):
    """后台抓帧线程：shot_fn -> 裁剪 -> 缩放 320x240 -> JPEG，缓存 latest。"""

    def __init__(self, args, shot_fn):
        super().__init__(daemon=True, name="screen_cast_capture")
        self.args = args
        self.shot_fn = shot_fn
        self.resample_name = getattr(args, "resample", "lanczos")
        self.saturation = max(0.0, float(getattr(args, "saturation", 1.0)))
        self.contrast = max(0.0, float(getattr(args, "contrast", 1.0)))
        self.cond = threading.Condition()
        self.latest = None      # bytes（JPEG）
        self.frame_id = 0
        self.stop_event = threading.Event()
        self.error = None
        self._content_size_logged = False

    def run(self):
        from PIL import Image, ImageEnhance
        resample = getattr(Image, RESAMPLE_MAP[self.resample_name])
        interval = 1.0 / max(1, self.args.fps)
        try:
            while not self.stop_event.is_set():
                start = time.time()
                img = self.shot_fn()
                iw, ih = img.size
                if iw > 0 and ih > 0:
                    # 保持源比例（如 16:9）缩放到 ≤320x240，再居中贴回 320x240 黑底画布。
                    # 设备 jpg.so 只解码 320x240 的帧（其余尺寸静默拒绝、无画面），
                    # 黑边由设备端用 content_h 裁剪成"透明"（见 video_renderer.lua）。
                    fit = min(TARGET_W / iw, TARGET_H / ih)
                    nw = max(1, round(iw * fit))
                    nh = max(1, round(ih * fit))
                    img = img.resize((nw, nh), resample)
                    # 颜色后处理：全息/透光屏透光会洗淡颜色，用饱和度/对比度补偿
                    # （--vivid 一键预设；--saturation/--contrast 单独调，默认 1.0 不处理）
                    if self.saturation != 1.0:
                        img = ImageEnhance.Color(img).enhance(self.saturation)
                    if self.contrast != 1.0:
                        img = ImageEnhance.Contrast(img).enhance(self.contrast)
                    canvas = Image.new("RGB", (TARGET_W, TARGET_H), (0, 0, 0))
                    canvas.paste(img, ((TARGET_W - nw) // 2, (TARGET_H - nh) // 2))
                    img = canvas
                    if not self._content_size_logged:
                        self._content_size_logged = True
                        bar = max(0, (TARGET_H - nh) // 2)
                        print(f"[screen_cast] 内容缩放后 {nw}x{nh}，居中贴到 {TARGET_W}x{TARGET_H} 黑底画布（上下黑边各 {bar}px）")
                        print(f"[screen_cast] 设备 WebUI 如需四周透出：把“内容高度 content_h”设为 {nh}（0=全屏显示黑边）")
                buf = io.BytesIO()
                save_kwargs = {"quality": self.args.quality, "optimize": self.args.optimize}
                if self.args.subsampling != "keep":
                    save_kwargs["subsampling"] = SUBSAMPLING_MAP[self.args.subsampling]
                img.save(buf, "JPEG", **save_kwargs)
                jpeg = buf.getvalue()
                with self.cond:
                    self.latest = jpeg
                    self.frame_id += 1
                    self.cond.notify_all()
                elapsed = time.time() - start
                delay = interval - elapsed
                if delay > 0:
                    self.stop_event.wait(delay)
        except Exception as exc:  # noqa: BLE001
            self.error = exc
            with self.cond:
                self.cond.notify_all()

    def stop(self):
        self.stop_event.set()
        with self.cond:
            self.cond.notify_all()


class SilentServer(ThreadingHTTPServer):
    """静默客户端主动断开连接时的 traceback（ConnectionResetError 等属正常现象）。"""

    def handle_error(self, request, client_address):
        exc = sys.exc_info()[1]
        if isinstance(exc, (ConnectionResetError, BrokenPipeError, ConnectionAbortedError, TimeoutError)):
            return
        super().handle_error(request, client_address)


class StreamHandler(BaseHTTPRequestHandler):
    capture = None

    def log_message(self, fmt, *args):  # 静音默认访问日志
        return

    def do_GET(self):
        path = urlparse(self.path).path
        if path in ("/", "/index.html"):
            self._serve_root()
        elif path == "/stream":
            self._serve_stream()
        else:
            self.send_error(404)

    def _serve_root(self):
        ip = lan_ip()
        port = self.server.server_address[1]
        html = f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>screen_cast 发送端预览</title>
<style>
 body{{margin:0;background:#0b1020;color:#dbe7f5;font:14px/1.5 system-ui,"Segoe UI",sans-serif;text-align:center}}
 h1{{font-size:18px;margin:14px 0 4px}}
 p{{color:#8a94a6;margin:4px 0}}
 img{{width:min(96vw,640px);border:1px solid #22334a;border-radius:8px;display:block;margin:14px auto}}
 .addr{{font:13px ui-monospace,Consolas,monospace;color:#71f59b}}
</style></head><body>
 <h1>screen_cast 发送端</h1>
 <p>局域网地址：<span class="addr">http://{ip}:{port}/stream</span></p>
 <p>设备 WebUI 里填：IP <b>{ip}</b>，端口 <b>{port}</b>，路径 <b>/stream</b></p>
 <img src="/stream" alt="MJPEG 预览">
</body></html>
"""
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(html.encode("utf-8"))

    def _serve_stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "multipart/x-mixed-replace; boundary=screen_cast_frame")
        self.send_header("Cache-Control", "no-cache, no-store")
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        capture = self.capture
        try:
            while True:
                with capture.cond:
                    # 有新帧立即返回；2 秒无新帧则重发最近一帧保活
                    capture.cond.wait(timeout=2.0)
                    frame = capture.latest
                    if frame is None:
                        if capture.error:
                            break
                        continue
                self.wfile.write(b"--" + BOUNDARY + b"\r\n")
                self.wfile.write(b"Content-Type: image/jpeg\r\n")
                self.wfile.write(b"Content-Length: %d\r\n\r\n" % len(frame))
                self.wfile.write(frame)
                self.wfile.write(b"\r\n")
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError, OSError):
            pass


def parse_args():
    parser = argparse.ArgumentParser(description="screen_cast 发送端：屏幕区域 -> 320x240 MJPEG 流")
    parser.add_argument("--monitor", type=int, default=0, help="显示器编号（0=主屏）")
    parser.add_argument("--x", type=int, default=None, help="抓取区域左上角 X")
    parser.add_argument("--y", type=int, default=None, help="抓取区域左上角 Y")
    parser.add_argument("--w", type=int, default=None, help="抓取区域宽度")
    parser.add_argument("--h", type=int, default=None, help="抓取区域高度")
    parser.add_argument("--port", type=int, default=8080, help="HTTP 端口（默认 8080）")
    parser.add_argument("--fps", type=int, default=15, help="目标帧率（默认 15）")
    parser.add_argument("--quality", type=int, default=85, help="JPEG 质量 1-100（默认 85）")
    parser.add_argument("--subsampling", default="444",
                        choices=["444", "422", "420", "keep"],
                        help="JPEG 色度子采样：444=色彩最准（默认，体积稍大） 422/420=体积更小但色彩略差 keep=跟随源图")
    parser.add_argument("--resample", default="lanczos",
                        choices=["nearest", "bilinear", "bicubic", "lanczos"],
                        help="缩放插值：lanczos=最清晰（默认） bicubic=较清晰 bilinear=柔和 nearest=锐利但易锯齿")
    parser.add_argument("--vivid", action="store_true",
                        help="全息/透光屏颜色增强预设：saturation=1.4 contrast=1.15（可再用 --saturation/--contrast 覆盖）")
    parser.add_argument("--saturation", type=float, default=None,
                        help="颜色饱和度倍率（>1 更鲜艳，全息屏建议 1.3–1.5；默认 1.0 不处理）")
    parser.add_argument("--contrast", type=float, default=None,
                        help="对比度倍率（>1 色彩更深，配合 saturation 用；默认 1.0 不处理）")
    parser.add_argument("--optimize", action="store_true",
                        help="启用 JPEG Huffman 优化（编码略慢，体积更小）")
    parser.add_argument("--url", default=None, help="（可选）headless 截取网页并推流，需 playwright")
    parser.add_argument("--page-w", type=int, default=1280, help="--url 模式的视口宽度")
    parser.add_argument("--page-h", type=int, default=720, help="--url 模式的视口高度")
    parser.add_argument("--ws", type=int, default=None, help="（Phase 2 预留）WebSocket 音频/麦克风端口")
    return parser.parse_args()


def main():
    args = parse_args()
    if args.w or args.h:
        args.x = args.x or 0
        args.y = args.y or 0
    # 颜色后处理默认值：--vivid 预设，或被 --saturation/--contrast 显式覆盖
    if args.saturation is None:
        args.saturation = 1.4 if args.vivid else 1.0
    if args.contrast is None:
        args.contrast = 1.15 if args.vivid else 1.0

    if args.url:
        shot_fn = make_url_shot(args)
    else:
        shot_fn = make_screen_shot(args)

    capture = CaptureThread(args, shot_fn)
    StreamHandler.capture = capture
    capture.start()

    try:
        server = SilentServer(("0.0.0.0", args.port), StreamHandler)
    except OSError as exc:
        raise SystemExit(f"无法监听端口 {args.port}：{exc}")

    ip = lan_ip()
    print("[screen_cast] 发送端已启动")
    print(f"[screen_cast] 本机局域网 IP：{ip}")
    print(f"[screen_cast] MJPEG 流：   http://{ip}:{args.port}/stream")
    print(f"[screen_cast] 预览页：     http://{ip}:{args.port}/")
    print(f"[screen_cast] 输出：       ≤{TARGET_W}x{TARGET_H} 保比例  {args.fps}fps  quality={args.quality}  subsampling={args.subsampling}  resample={args.resample}" + ("  optimize" if args.optimize else ""))
    if args.saturation != 1.0 or args.contrast != 1.0:
        print(f"[screen_cast] 颜色：      saturation={args.saturation:g}  contrast={args.contrast:g}")
    print("[screen_cast] 设备 WebUI 填：IP={} 端口={} 路径=/stream".format(ip, args.port))
    if args.ws:
        print(f"[screen_cast] 注：--ws {args.ws} 为 Phase 2 预留（音频/麦克风），当前版本未启用。")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[screen_cast] 正在退出...")
    finally:
        capture.stop()
        capture.join(timeout=1.0)
        server.shutdown()
        server.server_close()
        if capture.error:
            print(f"[screen_cast] 抓帧线程异常：{capture.error}")


if __name__ == "__main__":
    main()
