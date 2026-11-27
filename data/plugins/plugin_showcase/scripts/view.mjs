// 最小网页示例：零依赖的本地页面，由 script_serve 启动，宿主开局一个随机回环端口。
// 页面内容完全由插件自己决定，这里是给用户看的示范页。
import http from "node:http";

const page = `<!doctype html>
<html lang="zh-CN">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>插件示例 · 视图窗口</title>
<style>
  :root {
    --bg: #0d1320;
    --panel: #151d2c;
    --border: #24314a;
    --text: #e6eefc;
    --muted: #93a3b8;
    --accent: #7aa2ff;
    --canvas: #0f1726;
  }
  @media (prefers-color-scheme: light) {
    :root {
      --bg: #f5f8fd;
      --panel: #ffffff;
      --border: #dbe4f2;
      --text: #1b2434;
      --muted: #5c6b82;
      --accent: #2f6bff;
      --canvas: #eef3fc;
    }
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: system-ui, "Segoe UI", "Microsoft YaHei", sans-serif;
    line-height: 1.65;
  }
  main { max-width: 880px; margin: 0 auto; padding: 44px 28px 56px; }
  .eyebrow {
    margin: 0 0 6px;
    font-size: 12px;
    letter-spacing: .18em;
    text-transform: uppercase;
    color: var(--accent);
  }
  h1 { margin: 0 0 10px; font-size: 30px; line-height: 1.25; }
  .lead { margin: 0; max-width: 640px; color: var(--muted); }
  .steps {
    list-style: none;
    display: grid;
    gap: 14px;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    padding: 0;
    margin: 30px 0 24px;
  }
  .steps li {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 16px 18px;
  }
  .steps .no {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
  }
  .steps h2 { margin: 10px 0 4px; font-size: 15px; }
  .steps p { margin: 0; font-size: 13.5px; color: var(--muted); }
  .status {
    display: grid;
    gap: 12px;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 14px 18px;
    margin-bottom: 24px;
  }
  .status .label { display: block; font-size: 12px; color: var(--muted); }
  .status b { font-weight: 600; font-size: 14px; word-break: break-all; }
  .stage { margin: 0; }
  canvas {
    width: 100%;
    height: 360px;
    display: block;
    border-radius: 14px;
    background: var(--canvas);
    border: 1px solid var(--border);
    touch-action: none;
    cursor: grab;
  }
  canvas:active { cursor: grabbing; }
  .hint { margin: 10px 2px 0; font-size: 13px; color: var(--muted); }
  footer {
    margin-top: 34px;
    padding-top: 18px;
    border-top: 1px solid var(--border);
    font-size: 13px;
    color: var(--muted);
  }
  code {
    font-family: ui-monospace, Consolas, monospace;
    font-size: .92em;
    color: var(--accent);
  }
</style>
<main>
  <p class="eyebrow">插件示例 · 视图窗口</p>
  <h1>这个页面由插件自己提供</h1>
  <p class="lead">
    宿主在插件目录里启动一个 Node 脚本，拿到它打印的本地地址，再放进应用内的沙箱窗口。
    页面拿不到 LingChat 前端，也没有 Tauri 权限，只是普通网页。
  </p>

  <ol class="steps">
    <li>
      <span class="no">1</span>
      <h2>script_serve</h2>
      <p>运行 <code>scripts/view.mjs</code>，它监听本机随机端口，把地址打印到 stdout。</p>
    </li>
    <li>
      <span class="no">2</span>
      <h2>open_plugin_view</h2>
      <p>宿主确认地址是回环地址，然后在应用内开一个独立窗口加载它。</p>
    </li>
    <li>
      <span class="no">3</span>
      <h2>关窗即停</h2>
      <p>窗口关闭时，宿主会结束这条脚本服务，不在后台留下进程。</p>
    </li>
  </ol>

  <section class="status">
    <div><span class="label">页面地址</span><b id="origin">-</b></div>
    <div><span class="label">打开于</span><b id="opened">-</b></div>
    <div><span class="label">已运行</span><b id="uptime">0 秒</b></div>
  </section>

  <figure class="stage">
    <canvas id="view" width="720" height="360"></canvas>
    <p class="hint">按住拖动可以旋转。插件页面可以是任何东西，包括将来的建筑预览。</p>
  </figure>

  <footer>
    想写自己的插件？复制这个目录，改 <code>manifest.toml</code> 里的 id 和名字就能开始，
    详见 <code>docs/plugin-dev-guide.md</code>。
  </footer>
</main>
<script>
  var openedAt = Date.now();
  document.getElementById("origin").textContent = location.origin;
  document.getElementById("opened").textContent = new Date(openedAt).toLocaleTimeString("zh-CN", { hour12: false });

  var uptime = document.getElementById("uptime");
  function tickClock() {
    var seconds = Math.floor((Date.now() - openedAt) / 1000);
    uptime.textContent = seconds + " 秒";
    setTimeout(tickClock, 1000);
  }
  tickClock();

  var canvas = document.getElementById("view");
  var context = canvas.getContext("2d");
  var ratio = window.devicePixelRatio || 1;
  var width = 0;
  var height = 0;

  function resize() {
    var rect = canvas.getBoundingClientRect();
    width = rect.width;
    height = rect.height;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
  }
  window.addEventListener("resize", resize);
  resize();

  // 三层金字塔：底层 3x3，二层 2x2，顶层 1 格。
  var cells = [];
  var layers = [
    { y: 0, spots: [-1, 0, 1] },
    { y: 1, spots: [-0.5, 0.5] },
    { y: 2, spots: [0] }
  ];
  for (var l = 0; l < layers.length; l += 1) {
    for (var i = 0; i < layers[l].spots.length; i += 1) {
      for (var j = 0; j < layers[l].spots.length; j += 1) {
        cells.push({ x: layers[l].spots[i], y: layers[l].y, z: layers[l].spots[j] });
      }
    }
  }
  var palette = [
    { top: "#6d93f8", left: "#3d5aa8", right: "#4f70c4" },
    { top: "#7ea6ff", left: "#4767bd", right: "#5a7ed6" },
    { top: "#93b7ff", left: "#5273cb", right: "#688ce4" }
  ];

  var angle = 0.65;
  var dragging = false;
  var lastX = 0;

  canvas.addEventListener("pointerdown", function (event) {
    dragging = true;
    lastX = event.clientX;
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener("pointermove", function (event) {
    if (!dragging) return;
    angle += (event.clientX - lastX) * 0.01;
    lastX = event.clientX;
  });
  canvas.addEventListener("pointerup", function () {
    dragging = false;
  });
  canvas.addEventListener("pointercancel", function () {
    dragging = false;
  });

  function face(points, color) {
    context.fillStyle = color;
    context.beginPath();
    context.moveTo(points[0].x, points[0].y);
    for (var n = 1; n < points.length; n += 1) context.lineTo(points[n].x, points[n].y);
    context.closePath();
    context.fill();
  }

  function frame() {
    if (!dragging) angle += 0.0035;
    context.clearRect(0, 0, width, height);

    var size = Math.min(46, width / 16);
    var rise = size * 0.82;
    var cos = Math.cos(angle);
    var sin = Math.sin(angle);
    var centerX = width / 2;
    var centerY = height / 2 - size * 0.5;

    var projected = [];
    for (var n = 0; n < cells.length; n += 1) {
      var cell = cells[n];
      var rx = cell.x * cos - cell.z * sin;
      var rz = cell.x * sin + cell.z * cos;
      projected.push({
        x: cell.x,
        y: cell.y,
        z: cell.z,
        rx: rx,
        rz: rz,
        sx: centerX + (rx - rz) * size,
        sy: centerY + (rx + rz) * size * 0.5 - cell.y * rise
      });
    }
    projected.sort(function (a, b) {
      return (a.rx + a.rz) - (b.rx + b.rz) || a.y - b.y;
    });

    for (var k = 0; k < projected.length; k += 1) {
      var c = projected[k];
      var color = palette[Math.min(c.y, palette.length - 1)];
      var half = size / 2;
      var sx = c.sx;
      var sy = c.sy;
      face([
        { x: sx, y: sy },
        { x: sx + size / 2, y: sy + half },
        { x: sx, y: sy + size },
        { x: sx - size / 2, y: sy + half }
      ], color.top);
      face([
        { x: sx - size / 2, y: sy + half },
        { x: sx, y: sy + size },
        { x: sx, y: sy + size + rise },
        { x: sx - size / 2, y: sy + half + rise }
      ], color.left);
      face([
        { x: sx, y: sy + size },
        { x: sx + size / 2, y: sy + half },
        { x: sx + size / 2, y: sy + half + rise },
        { x: sx, y: sy + size + rise }
      ], color.right);
    }
    requestAnimationFrame(frame);
  }
  frame();
</script>
</html>`;

const server = http.createServer((_request, response) => {
  response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  response.end(page);
});

server.listen(0, "127.0.0.1", () => {
  const address = server.address();
  console.log(`Plugin view: http://127.0.0.1:${address.port}/`);
});
