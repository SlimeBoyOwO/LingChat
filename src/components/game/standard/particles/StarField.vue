<template>
  <canvas ref="canvasRef" class="starfield-canvas"></canvas>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch } from "vue";
import { startFrameLoop } from "@/core/animation/frame-scheduler";

/** 帧率上限：原先在帧循环里自己按 30fps 跳帧，现折算为共享调度器的 fps 选项 */
const FPS_LIMIT = 30;

const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
  starCount: {
    type: Number,
    default: 200,
  },
  scrollSpeed: {
    type: Number,
    default: 0.2,
  },
  colors: {
    type: Array,
    default: () => [
      "rgb(173, 216, 230)",
      "rgb(176, 224, 230)",
      "rgb(241, 141, 252)",
      "rgb(176, 230, 224)",
      "rgb(173, 230, 216)",
    ],
  },
});

const emit = defineEmits(["ready"]);

const canvasRef = ref(null);
const starField = ref(null);

// Star类定义
class StarFieldRenderer {
  constructor(canvas, config) {
    this.canvas = canvas;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Could not get canvas context");
    this.ctx = context;

    this.stars = [];
    this.config = {
      starCount: config.starCount,
      starSize: 2,
      attractorSize: 100,
      scrollSpeed: config.scrollSpeed,
      directionChangeRate: 0.2,
      colors: config.colors,
    };

    this.pointer = { x: 0, y: 0 };
    this.dir = Math.PI;
    this.w = 0;
    this.h = 0;

    /** 共享帧循环句柄（30fps 上限、页面隐藏/失焦暂停均由调度器统一处理） */
    this.loop = null;

    this.init();
  }

  init() {
    this.setupCanvas();
    this.createStars();
    this.addEventListeners();
    this.startAnimation();
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
      const color = this.config.colors[Math.floor(Math.random() * this.config.colors.length)];

      this.stars.push({
        x: this.randomInt(this.w * 1000),
        y: this.randomInt(this.h * 1000),
        v: z + 0.5,
        color,
      });
    }

    this.stars.sort((a, b) => a.v - b.v);
  }

  update = (now) => {
    // 帧率上限与页面隐藏/失焦暂停由共享调度器统一处理（见 frame-scheduler），
    // 这里只写「每帧做什么」，不再自己 requestAnimationFrame
    if (this.w !== window.innerWidth || this.h !== window.innerHeight) {
      this.setupCanvas();
    }

    // 方向相位使用绝对时间（虚拟时钟），保证暂停恢复后不跳变
    this.dir = Math.sin((now / 13289) * this.config.directionChangeRate) * Math.PI;
    this.ctx.clearRect(0, 0, this.w, this.h);
    this.ctx.globalCompositeOperation = "lighter";

    const dx = Math.cos(this.dir) * this.config.scrollSpeed;
    const dy = Math.sin(this.dir) * this.config.scrollSpeed;

    this.stars.forEach((star) => {
      star.x += star.v * dx;
      star.y += star.v * dy;

      const x =
        this.modulo(star.x, this.w + this.config.starSize + this.config.attractorSize) -
        (this.config.starSize / 2 + this.config.attractorSize / 2);
      const y =
        this.modulo(star.y, this.h + this.config.starSize + this.config.attractorSize) -
        (this.config.starSize / 2 + this.config.attractorSize / 2);

      this.ctx.fillStyle = star.color;
      this.ctx.fillRect(x, y, this.config.starSize * star.v, this.config.starSize * star.v);
    });

    this.ctx.globalCompositeOperation = "source-over";
  };

  addEventListeners() {
    window.addEventListener("resize", this.handleResize);
    this.canvas.addEventListener("mousemove", this.handleMouseMove);
  }

  handleResize = () => {
    // 画布尺寸跟随窗口；共享帧循环常驻，无需重启（重启会重置调度器虚拟时钟）
    this.setupCanvas();
  };

  handleMouseMove = (e) => {
    this.pointer.x = e.clientX;
    this.pointer.y = e.clientY;
  };

  startAnimation() {
    // 句柄复用：重复调用不会重复订阅
    this.loop ??= startFrameLoop(this.update, { fps: FPS_LIMIT });
  }

  stopAnimation() {
    // stop() 幂等，重复调用不会泄漏订阅者
    this.loop?.stop();
    this.loop = null;
  }

  destroy() {
    this.stopAnimation();
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

// 组件逻辑
onMounted(() => {
  if (canvasRef.value && props.enabled) {
    starField.value = new StarFieldRenderer(canvasRef.value, {
      starCount: props.starCount,
      scrollSpeed: props.scrollSpeed,
      colors: props.colors,
    });
    emit("ready", starField.value);
  }
});

onUnmounted(() => {
  if (starField.value) {
    starField.value.destroy();
  }
});

// 监听启用状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal && canvasRef.value && !starField.value) {
      starField.value = new StarFieldRenderer(canvasRef.value, {
        starCount: props.starCount,
        scrollSpeed: props.scrollSpeed,
        colors: props.colors,
      });
      emit("ready", starField.value);
    } else if (!newVal && starField.value) {
      starField.value.destroy();
      starField.value = null;
    }
  },
);

// 监听配置变化
watch(
  () => props.starCount,
  (newVal) => {
    if (starField.value) {
      starField.value.destroy();
      starField.value = new StarFieldRenderer(canvasRef.value, {
        starCount: newVal,
        scrollSpeed: props.scrollSpeed,
        colors: props.colors,
      });
      emit("ready", starField.value);
    }
  },
);

watch(
  () => props.scrollSpeed,
  (newVal) => {
    if (starField.value) {
      starField.value.config.scrollSpeed = newVal;
    }
  },
);

watch(
  () => props.colors,
  (newVal) => {
    if (starField.value) {
      starField.value.config.colors = newVal;
    }
  },
);
</script>

<style scoped>
.starfield-canvas {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: -1;
  pointer-events: none;
}
</style>
