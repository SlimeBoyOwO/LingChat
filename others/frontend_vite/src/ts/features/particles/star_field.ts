interface Star {
  x: number;
  y: number;
  v: number;
  color: string;
}

interface Config {
  starCount: number;
  starSize: number;
  attractorSize: number;
  scrollSpeed: number;
  directionChangeRate: number;
  colors: string[];
}

interface Pointer {
  x: number;
  y: number;
}

export class StarField {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private stars: Star[];
  private config: Config;
  private pointer: Pointer;
  private dir: number;
  private animationId: number | null;
  private w: number;
  private h: number;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Could not get canvas context");
    this.ctx = context;

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
    this.w = 0;
    this.h = 0;

    this.init();
  }

  private init(): void {
    this.setupCanvas();
    this.createStars();
    this.startAnimation();
    this.addEventListeners();
  }

  private setupCanvas(): void {
    this.w = this.canvas.width = window.innerWidth;
    this.h = this.canvas.height = window.innerHeight;
    this.pointer.x = this.w / 2;
    this.pointer.y = this.h / 2;
  }

  private createStars(): void {
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

  private update = (timestamp: number): void => {
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
    this.animationId = requestAnimationFrame(this.update);
  };

  private addEventListeners(): void {
    window.addEventListener("resize", this.handleResize);
    this.canvas.addEventListener("mousemove", this.handleMouseMove);
  }

  private handleResize = (): void => {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
    }
    this.setupCanvas();
    this.startAnimation();
  };

  private handleMouseMove = (e: MouseEvent): void => {
    this.pointer.x = e.clientX;
    this.pointer.y = e.clientY;
  };

  private startAnimation(): void {
    this.animationId = requestAnimationFrame(this.update);
  }

  public destroy(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
    }
    window.removeEventListener("resize", this.handleResize);
    this.canvas.removeEventListener("mousemove", this.handleMouseMove);
  }

  // Helper methods
  private randomInt(min: number, max?: number): number {
    if (max === undefined) {
      max = min;
      min = 0;
    }
    return Math.floor(Math.random() * (max - min) + min);
  }

  private randomG(): number {
    return Math.random() * Math.random() * Math.random();
  }

  private modulo(value: number, mod: number): number {
    return ((value % mod) + mod) % mod;
  }
}
