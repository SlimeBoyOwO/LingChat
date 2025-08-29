export class StarField {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.stars = [];
    this.config = {
      starCount: 200,
      starSize: 2,
      attractorSize: 100,
      scrollSpeed: 0.2,
      directionChangeRate: 0.2,
      colors: [
        "rgb(173, 216, 230)",
        "rgb(176, 224, 230)",
        "rgb(241, 141, 252)",
        "rgb(176, 230, 224)",
        "rgb(173, 230, 216)",
      ],
    };

    this.pointer = { x: 0, y: 0 };
    this.dir = Math.PI;
    this.animationId = null;
    this.init();
  }

  init() {
    this.setupCanvas();
    this.createStars();
    this.startAnimation();
    this.addEventListeners();
  }

  setupCanvas() {
    this.w = this.canvas.width = window.innerWidth;
    this.h = this.canvas.height = window.innerHeight;
    this.pointer.x = this.w / 2;
    this.pointer.y = this.h / 2;
  }

  createStars() {
    for (let i = 0; i < this.config.starCount; i++) {
      const z = this.randomG();
      const color =
        this.config.colors[
          Math.floor(Math.random() * this.config.colors.length)
        ];

      this.stars.push({
        x: this.randomInt(this.w * 1000),
        y: this.randomInt(this.h * 1000),
        v: z + 0.5,
        color,
      });
    }

    this.stars.sort((a, b) => a.v - b.v);
  }

  update(timestamp) {
    if (this.w !== window.innerWidth || this.h !== window.innerHeight) {
      this.setupCanvas();
    }

    this.dir =
      Math.sin((timestamp / 13289) * this.config.directionChangeRate) * Math.PI;
    this.ctx.clearRect(0, 0, this.w, this.h);
    this.ctx.globalCompositeOperation = "lighter";

    const dx = Math.cos(this.dir) * this.config.scrollSpeed;
    const dy = Math.sin(this.dir) * this.config.scrollSpeed;

    this.stars.forEach((star) => {
      star.x += star.v * dx;
      star.y += star.v * dy;

      const x =
        this.modulo(
          star.x,
          this.w + this.config.starSize + this.config.attractorSize
        ) -
        (this.config.starSize / 2 + this.config.attractorSize / 2);
      const y =
        this.modulo(
          star.y,
          this.h + this.config.starSize + this.config.attractorSize
        ) -
        (this.config.starSize / 2 + this.config.attractorSize / 2);

      this.ctx.fillStyle = star.color;
      this.ctx.fillRect(
        x,
        y,
        this.config.starSize * star.v,
        this.config.starSize * star.v
      );
    });

    this.ctx.globalCompositeOperation = "source-over";
    this.animationId = requestAnimationFrame(this.update.bind(this));
  }

  addEventListeners() {
    window.addEventListener("resize", () => {
      cancelAnimationFrame(this.animationId);
      this.setupCanvas();
      this.startAnimation();
    });

    this.canvas.addEventListener("mousemove", (e) => {
      this.pointer.x = e.clientX;
      this.pointer.y = e.clientY;
    });
  }

  startAnimation() {
    this.animationId = requestAnimationFrame(this.update.bind(this));
  }

  destroy() {
    cancelAnimationFrame(this.animationId);
    window.removeEventListener("resize", this.handleResize);
    this.canvas.removeEventListener("mousemove", this.handleMouseMove);
  }

  // Helper methods
  randomInt(min, max) {
    if (max === undefined) {
      max = min;
      min = 0;
    }
    return Math.floor(Math.random() * (max - min) + min);
  }

  randomG() {
    return Math.random() * Math.random() * Math.random();
  }

  modulo(value, mod) {
    return ((value % mod) + mod) % mod;
  }
}
