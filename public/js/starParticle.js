var ctx = canvas.getContext("2d");
const stars = [];
var dir = Math.PI;
var w;
var h;
const starCount = 200; // 粒子数量，可以根据需要调整
const starSize = 2; // 粒子大小，可以根据需要调整
const attractorSize = 100;
const starSize2 = starSize / 2;
const attractorSize2 = attractorSize / 2;
const scrollSpeed = 0.2;
const directionChangeRate = 0.2;
var widthModulo;
var heightModulo;
var grad;
var grad1;
const pointer = { x: 0, y: 0 };

// 星空颜色池
const starColors = [
  "rgb(173, 216, 230)", // 淡蓝色
  "rgb(176, 224, 230)", // 淡青色
  "rgb(241, 141, 252)", // 淡紫色
  "rgb(176, 230, 224)", // 淡青白色
  "rgb(173, 230, 216)", // 青蓝色
];

const randI = (min, max = min + (min = 0)) =>
  (Math.random() * (max - min) + min) | 0;
const rand = (min, max = min + (min = 0)) => Math.random() * (max - min) + min;
const randG = (p) => Math.random() * Math.random() * Math.random();
const ease = (v, p = 2) => Math.pow(v < 0 ? 0 : v > 1 ? 1 : v, p);
const doFor = (count, callback) => {
  var i = 0;
  while (i < count) {
    callback(i++);
  }
};

pageResize();
function pageResize() {
  w = canvas.width = innerWidth;
  h = canvas.height = innerHeight;
  widthModulo = w + starSize + attractorSize;
  heightModulo = h + starSize + attractorSize;

  // 设置 pointer 为中心点
  pointer.x = w / 2;
  pointer.y = h / 2;

  grad = ctx.createLinearGradient(0, 0, 0, h);
  grad1 = ctx.createRadialGradient(
    w / 2,
    h / 2,
    0,
    w / 2,
    h / 2,
    Math.min(attractorSize * 2, w, h) / 2
  );
  doFor(16, (i) => {
    const c = ((i / 15) * 32) | 0;
    grad1.addColorStop(
      ease(i / 15, 1 / 40),
      "rgba(0,0,0," + (1 - i / 15) + ")"
    );
    grad.addColorStop(ease(i / 15, 1 / 4), "rgb(0,0," + c + ")");
  });
  grad.addColorStop(0, "black");
  grad.addColorStop(1, "#003");
  grad1.addColorStop(0, "rgba(0,0,0,1)");
  grad1.addColorStop(1, "rgba(0,0,0,0)");
}

var i;
for (i = 0; i < starCount; i++) {
  var z = randG();
  // 从颜色池中随机选择一个颜色
  var col = starColors[randI(starColors.length)];
  stars.push({
    x: randI(w * 1000),
    y: randI(h * 1000),
    v: z + 0.5,
    col: col, // 使用随机选择的颜色
  });
}
stars.sort((a, b) => a.v - b.v); // sort from back to front

function animation(timer) {
  var star, dx, dy, x, y, i, xx, yy, d;
  if (w !== innerWidth || h !== innerHeight) {
    pageResize();
  }

  // set the direction to make stars move in long circles
  dir = Math.sin((timer / 13289) * directionChangeRate) * Math.PI;

  // 清除背景（不再填充黑色背景）
  ctx.clearRect(0, 0, w, h); // 使用 clearRect 来清除画布内容

  ctx.globalCompositeOperation = "lighter";

  // get direction vector
  dx = Math.cos(dir) * scrollSpeed;
  dy = Math.sin(dir) * scrollSpeed;
  for (i = 0; i < starCount; i += 1) {
    star = stars[i];
    ctx.fillStyle = star.col;
    // move the star
    star.x += star.v * dx;
    star.y += star.v * dy;
    // make sure that stars are rendered on the current viewport
    x =
      (((star.x % widthModulo) + widthModulo) % widthModulo) -
      (starSize2 + attractorSize2);
    y =
      (((star.y % heightModulo) + heightModulo) % heightModulo) -
      (starSize2 + attractorSize2);
    xx = pointer.x - x;
    yy = pointer.y - y;
    d = Math.sqrt(xx * xx + yy * yy);
    // x -= (xx / d) * attractorSize2 * star.v;
    // y -= (yy / d) * attractorSize2 * star.v;        绕开中心
    // draw the star
    ctx.fillRect(x, y, starSize * star.v, starSize * star.v);
  }
  ctx.globalCompositeOperation = "source-over";
  requestAnimationFrame(animation);
}
requestAnimationFrame(animation);
