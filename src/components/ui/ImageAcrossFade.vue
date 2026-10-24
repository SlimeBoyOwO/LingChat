<template>
  <div class="relative h-full w-full">
    <!-- 默认插槽：保留原有底层内容 -->
    <slot></slot>
    <!-- 叠层插槽：与两层图片共用 absolute inset-0 的绘制盒子，
         触摸区域等叠层不必猜测外部定位与图片取景的关系 -->
    <slot name="overlay"></slot>
    <!-- 两层轮流作为当前图片和待显示图片；新图准备好前，旧图始终保留 -->
    <div
      v-for="(layer, index) in layers"
      :key="index"
      :ref="
        (el) => {
          layerElements[index] = el as HTMLElement | null;
        }
      "
      class="image-fade-layer absolute inset-0 h-full w-full will-change-[opacity] backface-hidden"
      :style="{
        opacity: layer.opacity,
        zIndex: index === activeLayer ? 10 : 20,
      }"
    >
      <!-- 在实际显示元素上解码，避免预加载完成后换地址仍短暂丢失纹理 -->
      <img
        v-if="layer.url"
        :ref="
          (el) => {
            layerImages[index] = el as HTMLImageElement | null;
          }
        "
        :crossorigin="layer.crossOrigin ? 'anonymous' : undefined"
        :src="layer.url"
        class="pointer-events-none absolute"
        :style="imageStyle"
        alt=""
        draggable="false"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick, onBeforeUnmount } from "vue";
import { imageProfile, needsInstantSwap, type ImageProfile } from "@/utils/image-transition";

const props = withDefaults(
  defineProps<{ src: string; duration?: number; objectFit?: string; position?: string }>(),
  { duration: 300, objectFit: "contain", position: "center bottom" },
);

const layers = ref([
  { url: "", opacity: 0, crossOrigin: true },
  { url: "", opacity: 0, crossOrigin: true },
]);
// 保存显示层及其图片的 DOM 引用，用于解码和透明度动画。
const layerElements: (HTMLElement | null)[] = [];
const layerImages: (HTMLImageElement | null)[] = [];
const activeLayer = ref(0);
const masks: (ImageProfile | null)[] = [null, null];
let animation: Animation | null = null;
// 每次换图推进代次号，只有最后一次加载可以更新显示状态。
let generation = 0;
// 供角色演出等待图片就绪；不等待整段渐变结束。
let currentImageLoadPromise = Promise.resolve();
let resolveLoad: (() => void) | undefined;

const imageStyle = computed(() => {
  const height = /^auto (\d+(?:\.\d+)?%)$/.exec(props.objectFit)?.[1];
  if (height) {
    // 保留窄屏立绘按高度取景的规则，宽度允许按原比例溢出。
    const [x = "center", y = "center"] = props.position.split(/\s+/);
    const anchors: Record<string, string> = {
      left: "0%",
      right: "100%",
      top: "0%",
      bottom: "100%",
      center: "50%",
    };
    const left = anchors[x] ?? x;
    const top = anchors[y] ?? y;
    return {
      width: "auto",
      height,
      maxWidth: "none",
      maxHeight: "none",
      left,
      top,
      transform: `translate(calc(-1 * ${left}), calc(-1 * ${top}))`,
    };
  }
  return {
    width: "100%",
    height: "100%",
    maxWidth: "none",
    maxHeight: "none",
    objectFit: props.objectFit as "contain" | "cover",
    objectPosition: props.position,
  };
});

// 上一次渐变被打断时，保留更明显的一层，再整理为单层显示。
function settleLayers() {
  const old = activeLayer.value;
  const next = 1 - old;
  const opacity = (index: number) => {
    const element = layerElements[index];
    return element ? Number(getComputedStyle(element).opacity) : layers.value[index].opacity;
  };
  const nextOpacity = opacity(next);
  if (layers.value[next].url && nextOpacity >= 0.5) {
    activeLayer.value = next;
  }
  animation?.cancel();
  animation = null;
  for (let i = 0; i < layers.value.length; i++) {
    if (i === activeLayer.value && layers.value[i].url) layers.value[i].opacity = 1;
    else {
      layers.value[i] = { url: "", opacity: 0, crossOrigin: true };
      masks[i] = null;
    }
  }
}

async function decodeLayer(index: number, run: number): Promise<HTMLImageElement | null> {
  // 等待 Vue 把新地址与跨域设置更新到真实 DOM，再等待加载和解码。
  await nextTick();
  if (run !== generation) return null;
  const image = layerImages[index];
  if (!image) return null;
  try {
    await image.decode();
  } catch (error) {
    if (run !== generation) return null;
    if (!image.complete || !image.naturalWidth) {
      if (!layers.value[index].crossOrigin) throw error;
      // 外部图片未提供跨域响应头时，去掉跨域读取设置后重新加载同一显示元素。
      layers.value[index].crossOrigin = false;
      return decodeLayer(index, run);
    }
  }
  return run === generation ? image : null;
}

// 小差分和不透明背景沿用渐变；新图淡入期间，旧图在底层保持可见。
async function fade(index: number, to: number, duration: number) {
  const layer = layers.value[index];
  const from = layer.opacity;
  layer.opacity = to;
  const element = layerElements[index];
  if (!element || duration <= 0) {
    await nextTick();
    return;
  }
  const effect = element.animate([{ opacity: from }, { opacity: to }], {
    duration,
    easing: "ease-in-out",
  });
  animation = effect;
  try {
    await effect.finished;
  } finally {
    // 正常结束与中途取消都释放动画，避免旧动画继续控制透明度。
    effect.cancel();
    if (animation === effect) animation = null;
  }
}

async function updateImage(url: string, run: number, ready: () => void) {
  if (!url || url.toLowerCase() === "none") {
    // 空地址表示清空图片，同时丢弃两层的轮廓信息。
    for (let i = 0; i < layers.value.length; i++) {
      layers.value[i] = { url: "", opacity: 0, crossOrigin: true };
      masks[i] = null;
    }
    return;
  }
  if (layers.value[activeLayer.value].url === url) return;
  const old = activeLayer.value;
  const next = 1 - old;
  // 1. 将新地址交给隐藏层，并在实际显示元素上完成加载与解码。
  // 直接替换可见层的地址，即使已预加载，也可能在新纹理就绪前闪空白。
  layers.value[next] = { url, opacity: 0, crossOrigin: true };
  const image = await decodeLayer(next, run);
  if (!image || run !== generation) return;
  const profile = imageProfile(url, image);
  masks[next] = profile;
  const hasOld = !!layers.value[old].url;
  // 2. 根据透明轮廓与取景判断是否属于不同姿势，避免两种姿势叠在一起。
  const instant = hasOld && needsInstantSwap(masks[old], profile);
  const duration = matchMedia("(prefers-reduced-motion: reduce)").matches
    ? 0
    : Math.max(0, props.duration);
  if (duration === 0 || instant) {
    // 3. 不同姿势在同一次 DOM 更新中交换可见性，不改动可见图片的地址。
    layers.value[next].opacity = 1;
    layers.value[old].opacity = 0;
    activeLayer.value = next;
    await nextTick();
    return;
  }
  // 4. 其余情况开启淡入，待过渡结束后释放旧图。
  const transition = fade(next, 1, duration);
  // 新图开始显示即可通知角色播放动作，避免渐变比瞬切额外晚一段过渡时间。
  ready();
  await transition;
  if (run !== generation) return;
  activeLayer.value = next;
  layers.value[old] = { url: "", opacity: 0, crossOrigin: true };
  masks[old] = null;
}

watch(
  () => props.src,
  (url) => {
    const run = ++generation;
    settleLayers();
    // 新请求接管时释放上一次等待，旧加载结果会被代次号检查拦住。
    resolveLoad?.();
    currentImageLoadPromise = new Promise<void>((resolve) => {
      resolveLoad = resolve;
    });
    const done = resolveLoad!;
    void updateImage(url, run, done)
      .catch((error) => {
        if (run === generation) console.warn(`加载图片失败: ${url}`, error);
      })
      .finally(done);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  // 卸载后阻止迟到的加载更新，并释放动画和外部等待。
  generation++;
  animation?.cancel();
  resolveLoad?.();
});

defineExpose({ waitForLoad: () => currentImageLoadPromise });
</script>
