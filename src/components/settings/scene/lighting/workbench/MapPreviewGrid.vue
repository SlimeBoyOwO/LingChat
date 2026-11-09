<script setup lang="ts">
/**
 * 三格材质预览：原图 / 法线 / 深度。
 *
 * 三件事必须成立：
 * 1. **两格互不连坐**：法线生成失败不能连带深度也不出图，各自 try/catch、各自报状态。
 * 2. **失败要说人话**：显示「生成失败」+ 原因摘要 + 重试，而不是永远停在「生成中…」。
 * 3. **能手动接管**：法线/深度都支持上传自己的图（沙箱原本就有这两个槽），
 *    传了就用真图，清掉回到自动生成。
 */
import { onUnmounted, ref, watch } from "vue";
import { generatePseudoDepth, generatePseudoNormal } from "@/render/lighting";

const props = defineProps<{
  characterSrc?: string | null;
  /** 用户上传的法线图；空则自动生成 */
  normalSrc?: string | null;
  /** 用户上传的深度图；空则自动生成 */
  depthSrc?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:normalSrc", v: string | null): void;
  (e: "update:depthSrc", v: string | null): void;
}>();

type MapState = "idle" | "ok" | "fail" | "empty";

const autoNormal = ref("");
const autoDepth = ref("");
const normalState = ref<MapState>("idle");
const depthState = ref<MapState>("idle");
const normalError = ref("");
const depthError = ref("");

const normalInput = ref<HTMLInputElement | null>(null);
const depthInput = ref<HTMLInputElement | null>(null);

let token = 0;

function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

function errorText(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  // 浏览器原文里最有用的就是这几种，其余截断到 40 字
  if (/tainted/i.test(msg)) return "图片跨域受限（画布被污染）";
  if (/SecurityError/i.test(msg)) return "安全限制，无法读取像素";
  return msg.length > 40 ? `${msg.slice(0, 40)}…` : msg;
}

async function buildMaps(src: string | null | undefined) {
  const my = ++token;
  autoNormal.value = "";
  autoDepth.value = "";
  normalState.value = "idle";
  depthState.value = "idle";
  normalError.value = "";
  depthError.value = "";
  if (!src) {
    // 没有立绘不是「失败」，是还没选：直说，别让人对着「生成中…」等
    normalState.value = "empty";
    depthState.value = "empty";
    return;
  }

  const img = await loadImage(src);
  if (!img || my !== token) {
    if (my === token && src) {
      normalState.value = "fail";
      depthState.value = "fail";
      normalError.value = "立绘加载失败";
      depthError.value = "立绘加载失败";
    }
    return;
  }

  // 立绘原图 2304×3072，两个生成器各自 drawImage 一次会白读两遍 7M 像素；
  // 先缩到 220 长边的小 canvas，后面都在这张上算（快一个数量级）
  const LONG = 220;
  const ratio = img.naturalWidth / img.naturalHeight;
  const sw = Math.max(1, Math.round(ratio >= 1 ? LONG : LONG * ratio));
  const sh = Math.max(1, Math.round(ratio >= 1 ? LONG / ratio : LONG));
  const small = document.createElement("canvas");
  small.width = sw;
  small.height = sh;
  small.getContext("2d", { willReadFrequently: true })?.drawImage(img, 0, 0, sw, sh);
  if (my !== token) return;

  try {
    const normal = generatePseudoNormal(small, LONG);
    if (my !== token) return;
    autoNormal.value = normal.toDataURL("image/png");
    normalState.value = "ok";
  } catch (err) {
    if (my !== token) return;
    normalState.value = "fail";
    normalError.value = errorText(err);
  }

  // 让出一帧，避免法线+深度合成一个超长任务
  await new Promise<void>((r) => requestAnimationFrame(() => r()));
  if (my !== token) return;

  try {
    const depth = generatePseudoDepth(small, sw, sh);
    if (my !== token) return;
    autoDepth.value = depth.toDataURL("image/png");
    depthState.value = "ok";
  } catch (err) {
    if (my !== token) return;
    depthState.value = "fail";
    depthError.value = errorText(err);
  }
}

watch(() => props.characterSrc, buildMaps, { immediate: true });

// —— 上传接管 ——

const MAX_BYTES = 8 * 1024 * 1024;

async function onPick(kind: "normal" | "depth", e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;

  if (!file.type.startsWith("image/")) {
    if (kind === "normal") normalError.value = "不是图片文件";
    else depthError.value = "不是图片文件";
    return;
  }
  if (file.size > MAX_BYTES) {
    const msg = `文件超过 ${MAX_BYTES / 1024 / 1024}MB`;
    if (kind === "normal") normalError.value = msg;
    else depthError.value = msg;
    return;
  }

  const dataUrl = await new Promise<string | null>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => resolve(null);
    reader.readAsDataURL(file);
  });
  if (!dataUrl) return;

  // 尺寸校验：太小基本不可能是有效贴图
  const img = await loadImage(dataUrl);
  if (!img || img.naturalWidth < 16 || img.naturalHeight < 16) {
    const msg = "图片尺寸太小（<16px）";
    if (kind === "normal") normalError.value = msg;
    else depthError.value = msg;
    return;
  }

  if (kind === "normal") emit("update:normalSrc", dataUrl);
  else emit("update:depthSrc", dataUrl);
}

function clear(kind: "normal" | "depth") {
  if (kind === "normal") emit("update:normalSrc", null);
  else emit("update:depthSrc", null);
}

function retry() {
  buildMaps(props.characterSrc);
}

onUnmounted(() => {
  token++;
});
</script>

<template>
  <div class="map-grid">
    <!-- 原图 -->
    <div class="map-cell">
      <div class="map-thumb">
        <img v-if="characterSrc" :src="characterSrc" alt="原图" />
        <span v-else class="map-ph">无立绘</span>
      </div>
      <span class="map-label">原图</span>
    </div>

    <!-- 法线 -->
    <div class="map-cell">
      <div class="map-thumb">
        <img v-if="normalSrc" :src="normalSrc" alt="法线（上传）" />
        <img v-else-if="normalState === 'ok'" :src="autoNormal" alt="法线（自动）" />
        <span v-else-if="normalState === 'empty'" class="map-ph">未选择立绘</span>
        <span v-else-if="normalState === 'fail'" class="map-ph map-ph-fail" :title="normalError">
          生成失败
        </span>
        <span v-else class="map-ph">生成中…</span>

        <div class="map-acts">
          <button type="button" class="map-act" title="上传法线图" @click="normalInput?.click()">
            上传
          </button>
          <button
            v-if="normalSrc"
            type="button"
            class="map-act"
            title="回到自动生成"
            @click="clear('normal')"
          >
            自动
          </button>
        </div>
      </div>
      <span class="map-label" :title="normalError">
        {{
          normalSrc
            ? "法线（上传）"
            : normalState === "fail"
              ? "法线（失败）"
              : normalState === "empty"
                ? "法线"
                : "法线（自动）"
        }}
      </span>
      <button
        v-if="normalState === 'fail' || normalSrc"
        type="button"
        class="map-retry"
        @click="normalState === 'fail' ? retry() : clear('normal')"
      >
        {{ normalState === "fail" ? "重试" : "恢复自动" }}
      </button>
      <input
        ref="normalInput"
        type="file"
        accept="image/*"
        hidden
        @change="onPick('normal', $event)"
      />
    </div>

    <!-- 深度 -->
    <div class="map-cell">
      <div class="map-thumb">
        <img v-if="depthSrc" :src="depthSrc" alt="深度（上传）" />
        <img v-else-if="depthState === 'ok'" :src="autoDepth" alt="深度（自动）" />
        <span v-else-if="depthState === 'empty'" class="map-ph">未选择立绘</span>
        <span v-else-if="depthState === 'fail'" class="map-ph map-ph-fail" :title="depthError">
          生成失败
        </span>
        <span v-else class="map-ph">生成中…</span>

        <div class="map-acts">
          <button type="button" class="map-act" title="上传深度图" @click="depthInput?.click()">
            上传
          </button>
          <button
            v-if="depthSrc"
            type="button"
            class="map-act"
            title="回到自动生成"
            @click="clear('depth')"
          >
            自动
          </button>
        </div>
      </div>
      <span class="map-label" :title="depthError">
        {{
          depthSrc
            ? "深度（上传）"
            : depthState === "fail"
              ? "深度（失败）"
              : depthState === "empty"
                ? "深度"
                : "深度（自动）"
        }}
      </span>
      <button
        v-if="depthState === 'fail' || depthSrc"
        type="button"
        class="map-retry"
        @click="depthState === 'fail' ? retry() : clear('depth')"
      >
        {{ depthState === "fail" ? "重试" : "恢复自动" }}
      </button>
      <input
        ref="depthInput"
        type="file"
        accept="image/*"
        hidden
        @change="onPick('depth', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.map-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
  padding: 2px 10px 6px;
}

.map-cell {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.map-thumb {
  position: relative;
  aspect-ratio: 1 / 1;
  border-radius: var(--wb-r-md);
  border: 1px solid var(--wb-border);
  background: repeating-conic-gradient(#f2f2f6 0% 25%, #fafafc 0% 50%) 50% / 12px 12px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

.map-thumb img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.map-ph {
  font-size: 9.5px;
  color: var(--wb-faint);
}

.map-ph-fail {
  color: #c0453f;
  font-weight: 600;
}

.map-acts {
  position: absolute;
  inset: auto 0 0 0;
  display: flex;
  justify-content: center;
  gap: 3px;
  padding: 3px;
  opacity: 0;
  transition: opacity 0.12s ease;
}

.map-thumb:hover .map-acts {
  opacity: 1;
}

.map-act {
  height: 16px;
  padding: 0 5px;
  border-radius: 4px;
  background: rgb(23 23 28 / 82%);
  color: #fff;
  font-size: 9.5px;
  line-height: 16px;
}

.map-act:hover {
  background: var(--wb-accent);
}

.map-label {
  font-size: 9.5px;
  color: var(--wb-muted);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.map-retry {
  font-size: 9.5px;
  color: var(--wb-accent);
  text-align: center;
  padding: 0;
  line-height: 1.2;
}

.map-retry:hover {
  text-decoration: underline;
}
</style>
