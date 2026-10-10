<template>
  <!-- 全屏独立层而不是弹窗内的一个标签页：外层的 click 关闭让「按下与抬起落在不同元素」
       的手势变成关闭弹窗，而拖顶点几乎必然滑出面板 -->
  <div class="fixed inset-0 z-[60] flex flex-col bg-black/70 text-white backdrop-blur-sm">
    <div class="flex flex-wrap items-center gap-3 border-b border-white/10 bg-white/5 px-5 py-3">
      <h3 class="m-0 text-base font-bold">{{ $t("settings.characterInfo.touch.editorTitle") }}</h3>
      <select v-model="costume" class="live2d-select">
        <option v-for="option in costumeOptions" :key="option" :value="option">{{ option }}</option>
      </select>
      <span class="hidden text-xs text-white/40 lg:inline">{{
        $t("settings.characterInfo.touch.editorHint")
      }}</span>
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="live2d-btn"
          :class="{ 'bg-cyan-400/20! text-cyan-100!': testMode }"
          :disabled="!costumes"
          :aria-pressed="testMode"
          @click="toggleTestMode"
        >
          {{ t(testMode ? "settings.touchTest.backToEdit" : "settings.touchTest.enter") }}
        </button>
        <button class="live2d-btn" :disabled="testMode || !canUndo" @click="undo">
          {{ $t("settings.characterInfo.touch.undo") }}
        </button>
        <button class="live2d-btn" @click="close">
          {{ $t("settings.characterInfo.touch.cancel") }}
        </button>
        <button class="live2d-btn-primary" :disabled="!costumes" @click="apply">
          {{ $t("settings.characterInfo.touch.apply") }}
        </button>
      </div>
    </div>

    <div class="flex min-h-0 flex-1 flex-col md:flex-row">
      <div class="flex min-h-48 min-w-0 flex-1 items-center justify-center p-3 md:p-6">
        <!-- 只按有没有立绘分支：预览里那个盒子既是画布也是量取景的元素，解析要等它量出来，
             把它一起挡在条件后面就会成环 -->
        <div v-if="!avatarUrl" class="text-sm text-white/50">
          {{ $t("settings.characterInfo.touch.missingAvatar") }}
        </div>
        <!-- self-stretch 是必需的：光有 aspect-ratio 没有一边是确定尺寸的话，空 div 作为
             flex item 会被撑成 0×0，量出来的取景恒为 0、区域永远解析不出来 -->
        <div
          v-else
          class="relative max-w-full self-stretch overflow-hidden rounded-2xl border border-white/10 bg-black/30"
          :style="{ aspectRatio: String(previewAspect) }"
        >
          <ImageAcrossFade
            class="absolute top-0 left-0 h-[102%] w-full"
            :src="avatarUrl"
            :object-fit="objectFit"
            position="center bottom"
          >
            <template #overlay>
              <div
                ref="overlayRef"
                class="absolute inset-0 z-30 overflow-hidden"
                :class="{ 'cursor-crosshair': testMode }"
                @click="testMode && testCanvasClick($event)"
              >
                <!-- viewBox 取 1×1 且 preserveAspectRatio 为 none：这个盒子已被摆成立绘的
                     绘制矩形，于是 user 坐标就是图片归一化坐标 -->
                <div :style="imageRectStyle" class="absolute">
                  <svg
                    class="absolute inset-0 h-full w-full"
                    viewBox="0 0 1 1"
                    preserveAspectRatio="none"
                    @click="handleCanvasClick"
                    @dblclick="handleCanvasDoubleClick"
                  >
                    <polygon
                      v-for="shape in shapes"
                      :key="shape.key"
                      :points="toPoints(shape.points)"
                      vector-effect="non-scaling-stroke"
                      class="region-shape"
                      :class="[
                        shape.selected ? 'region-shape-selected' : '',
                        testMode ? 'region-shape-testing' : '',
                      ]"
                      :stroke="shape.color"
                      :style="{ color: shape.color }"
                      @click.stop="!testMode && selectRegion(shape.part)"
                    />
                    <polyline
                      v-if="!testMode && draftPoints.length > 1"
                      :points="toPoints(draftPoints)"
                      vector-effect="non-scaling-stroke"
                      class="region-draft"
                    />
                  </svg>

                  <span
                    v-if="
                      testMode &&
                      testPoint &&
                      testPoint[0] >= 0 &&
                      testPoint[0] <= 1 &&
                      testPoint[1] >= 0 &&
                      testPoint[1] <= 1
                    "
                    class="pointer-events-none absolute z-40 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-cyan-400 shadow-[0_0_12px_rgba(34,211,238,0.8)]"
                    :style="{ left: `${testPoint[0] * 100}%`, top: `${testPoint[1] * 100}%` }"
                  />
                  <button
                    v-for="handle in handles"
                    :key="handle.key"
                    class="region-handle"
                    :class="handle.selected ? 'region-handle-selected' : ''"
                    :style="{ left: `${handle.x * 100}%`, top: `${handle.y * 100}%` }"
                    @pointerdown="startVertexDrag($event, handle)"
                  ></button>
                </div>
              </div>
            </template>
          </ImageAcrossFade>
        </div>
      </div>

      <div
        class="flex max-h-[45%] min-h-0 w-full shrink-0 flex-col gap-3 overflow-y-auto border-t border-white/10 bg-black/20 p-4 md:max-h-none md:w-80 md:border-t-0 md:border-l"
      >
        <div
          v-if="testMode"
          class="space-y-3 rounded-xl border border-cyan-200/20 bg-cyan-400/5 p-3"
          role="status"
        >
          <h4 class="text-sm font-semibold text-cyan-100">{{ t("settings.touchTest.title") }}</h4>
          <p class="text-xs leading-relaxed text-white/55">{{ t("settings.touchTest.hint") }}</p>
          <p v-if="draft" class="text-xs text-amber-100/75">
            {{ t("settings.touchTest.draftHint") }}
          </p>
          <p v-if="!testPoint" class="text-xs text-white/65">
            {{ t("settings.touchTest.clickHint") }}
          </p>
          <template v-else>
            <p class="text-xs text-white/50">
              {{
                t("settings.touchTest.coordinates", {
                  x: testPoint[0].toFixed(3),
                  y: testPoint[1].toFixed(3),
                })
              }}
            </p>
            <p class="text-sm text-cyan-100">
              {{
                hitPart
                  ? t("settings.touchTest.hit", { name: hitPart })
                  : t("settings.touchTest.miss")
              }}
            </p>
            <template v-if="hitPart">
              <p v-if="testHits.length > 1" class="text-xs text-amber-100/80">
                {{ t("settings.touchTest.overlap", { names: testHits.join(" → ") }) }}
              </p>
              <p class="text-xs text-white/50">{{ t("settings.touchTest.message") }}</p>
              <p
                class="rounded-lg bg-black/20 p-2 text-sm break-words whitespace-pre-wrap text-white/80"
              >
                {{ testMessage }}
              </p>
              <p v-if="!currentParts[hitPart]?.message" class="text-xs text-white/45">
                {{ t("settings.touchTest.fallback") }}
              </p>
            </template>
          </template>
        </div>
        <button v-if="costumes && !testMode" class="live2d-btn" @click="startNewRegion()">
          {{ $t("settings.characterInfo.touch.addRegion") }}
        </button>

        <fieldset
          v-for="(region, part) in currentParts"
          :key="part"
          :disabled="testMode"
          class="min-w-0 space-y-2 rounded-xl border p-3 transition-colors"
          :class="
            part === (testMode ? hitPart : selectedPart)
              ? 'border-[#5e72e4] bg-white/10'
              : 'border-white/10 bg-white/5'
          "
          @click="!testMode && (selectedPart = String(part))"
        >
          <div class="flex items-center gap-2">
            <!-- 名字不能直接绑 part：key 是改名的结果而不是输入源。Vue 每次重渲染都会把
                 DOM 的 value 强制写回绑定值，绑 part 的话，只要别处触发一次重渲染
                 （比如在右边填提示词），刚敲进去的名字就会被抹掉 -->
            <input
              :ref="(el) => registerNameInput(String(part), el as HTMLInputElement | null)"
              :value="nameDraft(part)"
              :placeholder="$t('settings.characterInfo.touch.namePlaceholder')"
              list="touch-part-names"
              class="form-control min-w-0 flex-1 rounded-lg border border-white/10 bg-black/20 px-2 py-1.5 text-sm"
              @input="onNameInput(String(part), ($event.target as HTMLInputElement).value)"
              @change="commitName(String(part))"
            />
            <button class="live2d-icon-btn" @click.stop="removeRegion(String(part))">x</button>
          </div>
          <textarea
            v-model="region.message"
            rows="2"
            :placeholder="$t('settings.characterInfo.touch.messagePlaceholder')"
            class="form-control w-full rounded-lg border border-white/10 bg-black/20 px-2 py-1.5 text-sm"
            @input="dirty = true"
          ></textarea>
          <div class="flex flex-wrap items-center gap-2">
            <span
              v-for="(polygon, index) in region.polygons"
              :key="index"
              class="flex items-center gap-1 rounded-lg bg-black/30 px-2 py-1 text-xs text-white/70"
            >
              {{ $t("settings.characterInfo.touch.polygon", { count: polygon.length }) }}
              <button class="live2d-icon-btn" @click.stop="removePolygon(String(part), index)">
                x
              </button>
            </span>
            <button class="live2d-btn" @click.stop="startAppendPolygon(String(part))">
              {{ $t("settings.characterInfo.touch.addPolygon") }}
            </button>
          </div>
        </fieldset>

        <!-- 有 URL 却始终解析不出来，说明文件读不到或不是图片：此时禁止保存，
             否则旧数据会在写回新形状时被静默丢掉 -->
        <p v-if="avatarUrl && !costumes" class="py-6 text-center text-xs text-white/40">
          {{ $t("settings.characterInfo.touch.unreadableAvatar") }}
        </p>
        <p
          v-else-if="costumes && !Object.keys(currentParts).length"
          class="py-6 text-center text-xs text-white/40"
        >
          {{ $t("settings.characterInfo.touch.empty") }}
        </p>
      </div>
    </div>

    <datalist id="touch-part-names">
      <option v-for="name in PART_PRESETS" :key="name" :value="name"></option>
    </datalist>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toRaw, watch } from "vue";
import { useI18n } from "vue-i18n";

import ImageAcrossFade from "@/components/ui/ImageAcrossFade.vue";
import { avatarObjectFit, loadImageAspect } from "@/composables/role/useRoleAvatar";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { useUIStore } from "@/stores/modules/ui/ui";
import {
  DEFAULT_COSTUME,
  fitFromObjectFit,
  hitRegion,
  hitRegions,
  touchRegionMessage,
  imageRectInBox,
  parseBodyPart,
  resolveCostumeKey,
  serializeBodyPart,
  type CostumeRegions,
  type LegacyFrame,
  type TouchPolygon,
  type TouchCostumes,
  type Vec2,
} from "@/components/game/standard/touch-regions";

interface ClothesOption {
  title: string;
  avatar: string;
}

const props = defineProps<{
  bodyPart: unknown;
  clothes: ClothesOption[];
  userName?: string;
}>();

const emit = defineEmits<{
  apply: [value: Record<string, unknown>];
  close: [];
}>();

/** 与 live2d-touch.ts 的 TOUCH_PART_ORDER 同一套词表，作者直觉可迁移 */
const PART_PRESETS = ["head", "body", "legs", "earLeft", "earRight"];
const PART_COLORS = ["#5e72e4", "#2dce89", "#fb6340", "#f5365c", "#11cdef", "#ffd600"];
const HISTORY_LIMIT = 20;

const { t } = useI18n();
const uiStore = useUIStore();
const dialogStore = useDialogStore();

const overlayRef = ref<HTMLDivElement | null>(null);
const costumes = ref<TouchCostumes | null>(null);
/** 初始服装必须独立于 frame 定下来：frame 要等立绘自然尺寸，而立绘要等服装选定，先有服装才不成环 */
const costume = ref(resolveCostumeKey(props.clothes.find((item) => item.avatar)?.title));
const selectedPart = ref<string | null>(null);
const selectedVertex = ref<{ part: string; polygon: number; vertex: number } | null>(null);
const draft = ref<{ part: string | null; points: TouchPolygon } | null>(null);
const imageAspect = ref(0);
const boxAspect = ref(0);
const dirty = ref(false);
const testMode = ref(false);
const testPoint = ref<Vec2 | null>(null);
const testHits = computed(() => {
  const point = testPoint.value;
  if (!point || point[0] < 0 || point[0] > 1 || point[1] < 0 || point[1] > 1) return [];
  return hitRegions(currentParts.value, point[0], point[1]);
});
const hitPart = computed(() => testHits.value[0] ?? null);
const testMessage = computed(() =>
  hitPart.value
    ? touchRegionMessage(currentParts.value[hitPart.value]?.message, props.userName ?? "")
    : "",
);

function toggleTestMode() {
  testMode.value = !testMode.value;
  testPoint.value = null;
  selectedVertex.value = null;
}
function testCanvasClick(event: MouseEvent) {
  testPoint.value = toImagePoint(event.clientX, event.clientY, false);
}
watch(costume, () => {
  testPoint.value = null;
});

const history: TouchCostumes[] = [];
const canUndo = ref(false);

/** 视口尺寸还没量出来时 aspectRatio 可能是 0 或 NaN，会让预览盒子退化成零宽 */
const previewAspect = computed(() => {
  const ratio = uiStore.aspectRatio;
  return Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
});

const objectFit = computed(() => avatarObjectFit(previewAspect.value));

/** 服装列表：游戏认的那份为准，另外补上 body_part 里已经存在但列表里没有的孤儿服装 */
const costumeOptions = computed(() => {
  const titles = props.clothes
    .filter((item) => item.avatar)
    .map((item) => resolveCostumeKey(item.title));
  for (const key of Object.keys(costumes.value ?? {})) {
    if (!titles.includes(key)) titles.push(key);
  }
  if (!titles.length) titles.push(DEFAULT_COSTUME);
  return titles;
});

const avatarUrl = computed(
  () => props.clothes.find((item) => resolveCostumeKey(item.title) === costume.value)?.avatar ?? "",
);

const frame = computed<LegacyFrame | null>(() =>
  boxAspect.value && imageAspect.value
    ? {
        boxAspect: boxAspect.value,
        imageAspect: imageAspect.value,
        fit: fitFromObjectFit(objectFit.value),
      }
    : null,
);

const imageRectStyle = computed(() => {
  if (!boxAspect.value || !imageAspect.value) return undefined;
  const rect = imageRectInBox(
    boxAspect.value,
    imageAspect.value,
    fitFromObjectFit(objectFit.value),
  );
  return {
    left: `${rect.x * 100}%`,
    top: `${rect.y * 100}%`,
    width: `${rect.width * 100}%`,
    height: `${rect.height * 100}%`,
  };
});

const currentParts = computed<CostumeRegions>(() => costumes.value?.[costume.value] ?? {});

const draftPoints = computed<TouchPolygon>(() => draft.value?.points ?? []);

interface Shape {
  key: string;
  part: string;
  points: TouchPolygon;
  color: string;
  selected: boolean;
}

const shapes = computed<Shape[]>(() => {
  const result: Shape[] = [];
  Object.entries(currentParts.value).forEach(([part, region], partIndex) => {
    const color = PART_COLORS[partIndex % PART_COLORS.length]!;
    region.polygons.forEach((points, polygonIndex) => {
      result.push({
        key: `${part}-${polygonIndex}`,
        part,
        points,
        color,
        selected: part === (testMode.value ? hitPart.value : selectedPart.value),
      });
    });
  });
  return result;
});

interface Handle {
  key: string;
  part: string;
  polygon: number;
  vertex: number;
  x: number;
  y: number;
  selected: boolean;
}

/** 只给选中的区域画手柄；未选中的只画轮廓，否则多个区域叠在一起会糊成一片 */
const handles = computed<Handle[]>(() => {
  if (testMode.value) return [];
  const result: Handle[] = [];
  const part = selectedPart.value;
  const region = part ? currentParts.value[part] : undefined;
  region?.polygons.forEach((points, polygonIndex) => {
    points.forEach(([x, y], vertex) => {
      result.push({
        key: `${part}-${polygonIndex}-${vertex}`,
        part: part!,
        polygon: polygonIndex,
        vertex,
        x,
        y,
        selected:
          selectedVertex.value?.part === part &&
          selectedVertex.value.polygon === polygonIndex &&
          selectedVertex.value.vertex === vertex,
      });
    });
  });
  draft.value?.points.forEach(([x, y], vertex) => {
    result.push({
      key: `draft-${vertex}`,
      part: "",
      polygon: -1,
      vertex,
      x,
      y,
      selected: vertex === 0,
    });
  });
  return result;
});

const toPoints = (points: TouchPolygon) => points.map(([x, y]) => `${x},${y}`).join(" ");

function pushHistory() {
  if (!costumes.value) return;
  history.push(structuredClone(toRaw(costumes.value)));
  if (history.length > HISTORY_LIMIT) history.shift();
  canUndo.value = history.length > 0;
  dirty.value = true;
}

function undo() {
  const previous = history.pop();
  canUndo.value = history.length > 0;
  if (previous) {
    costumes.value = previous;
    dirty.value = true;
  }
}

function ensureParts(): CostumeRegions {
  const table = costumes.value ?? (costumes.value = {});
  table[costume.value] ??= {};
  return table[costume.value]!;
}

function uniquePartName(base: string, taken: string[], self?: string): string {
  const used = new Set(taken.filter((name) => name !== self));
  if (!used.has(base)) return base;
  let index = 2;
  while (used.has(`${base}_${index}`)) index += 1;
  return `${base}_${index}`;
}

/** 输入框里正在编辑的名字。它与 part（键）分开存，正是为了让重渲染不会把用户敲的字冲掉。 */
const nameDrafts = ref<Record<string, string>>({});

function nameDraft(part: string): string {
  return nameDrafts.value[part] ?? part;
}

function onNameInput(part: string, value: string) {
  nameDrafts.value[part] = value;
}

function commitName(part: string) {
  const draft = nameDrafts.value[part];
  delete nameDrafts.value[part];
  if (draft !== undefined) renamePart(part, draft);
}

const nameInputs = new Map<string, HTMLInputElement>();

function registerNameInput(part: string, element: HTMLInputElement | null) {
  if (element) nameInputs.set(part, element);
  else nameInputs.delete(part);
}

/** 新增区域后把焦点放到名字上，先命名再画 */
async function focusNameInput(part: string) {
  await nextTick();
  nameInputs.get(part)?.select();
}

function renamePart(from: string, rawTo: string) {
  const to = rawTo.trim();
  const parts = currentParts.value;
  if (!to || to === from || !parts[from]) return;
  pushHistory();
  const name = uniquePartName(to, Object.keys(parts), from);
  // 原地重建而不是删了再塞：新键会被追加到对象末尾，卡片会跳到列表最下面
  const entries = Object.entries(parts).map(
    ([key, region]) => [key === from ? name : key, region] as const,
  );
  for (const key of Object.keys(parts)) delete parts[key];
  for (const [key, region] of entries) parts[key] = region;
  if (selectedPart.value === from) selectedPart.value = name;
}

function removeRegion(part: string) {
  pushHistory();
  delete currentParts.value[part];
  if (selectedPart.value === part) selectedPart.value = null;
}

function removePolygon(part: string, index: number) {
  pushHistory();
  currentParts.value[part]?.polygons.splice(index, 1);
  if (!currentParts.value[part]?.polygons.length) delete currentParts.value[part];
}

/**
 * 新增区域：先把区域建出来并聚焦名字，名字定了再画。
 *
 * 之前是先画后命名，名字只能等闭合时补一个默认值，作者得回头去改；而且那时区域还没有
 * 稳定的键，改起来更绕。空区域落盘时会被丢掉，所以只命名不画不会留下垃圾。
 */
function startNewRegion(firstPoint?: Vec2) {
  closeDraft();
  if (!costumes.value) return;
  pushHistory();
  const parts = ensureParts();
  const name = uniquePartName(t("settings.characterInfo.touch.newRegion"), Object.keys(parts));
  parts[name] = { message: "", polygons: [] };
  selectedPart.value = name;
  draft.value = { part: name, points: firstPoint ? [firstPoint] : [] };
  void focusNameInput(name);
}

function startAppendPolygon(part: string) {
  closeDraft();
  selectedPart.value = part;
  draft.value = { part, points: [] };
}

function closeDraft() {
  const active = draft.value;
  draft.value = null;
  if (!active?.part || active.points.length < 3) return;
  const region = currentParts.value[active.part];
  if (!region) return;
  pushHistory();
  region.polygons.push(active.points);
  selectedPart.value = active.part;
}

function selectRegion(part: string) {
  if (draft.value) return;
  selectedPart.value = part;
  selectedVertex.value = null;
}

/** 视口坐标换算到图片归一化坐标，与 SVG 的盒子互为逆运算 */
function toImagePoint(clientX: number, clientY: number, clamp = true): Vec2 | null {
  const element = overlayRef.value;
  if (!element || !imageAspect.value || !boxAspect.value) return null;
  const box = element.getBoundingClientRect();
  if (!box.width || !box.height) return null;
  const rect = imageRectInBox(
    boxAspect.value,
    imageAspect.value,
    fitFromObjectFit(objectFit.value),
  );
  const x = ((clientX - box.left) / box.width - rect.x) / rect.width;
  const y = ((clientY - box.top) / box.height - rect.y) / rect.height;
  return clamp ? [Math.max(0, Math.min(1, x)), Math.max(0, Math.min(1, y))] : [x, y];
}

function handleCanvasClick(event: MouseEvent) {
  if (testMode.value) return;
  // 还没解析出来就不许画：此时画下去会把 costumes 从 null 顶成空表，
  // 应用时用空表覆盖旧数据
  if (!costumes.value) return;
  const point = toImagePoint(event.clientX, event.clientY);
  if (!point) return;
  if (draft.value) {
    draft.value.points.push(point);
    return;
  }
  const part = hitRegion(currentParts.value, point[0], point[1]);
  if (part) {
    selectRegion(part);
    return;
  }
  // 空白处：有选中的区域就往它上面再加一块，否则按「新增区域」的流程起一个
  if (selectedPart.value) {
    draft.value = { part: selectedPart.value, points: [point] };
    return;
  }
  startNewRegion(point);
}

function handleCanvasDoubleClick() {
  if (testMode.value) return;
  // 双击会先派发两次 click，末尾多出一个几乎重合的点，去掉再闭合
  const points = draft.value?.points;
  if (points && points.length >= 2) {
    const [lastX, lastY] = points[points.length - 1]!;
    const [prevX, prevY] = points[points.length - 2]!;
    if (Math.hypot(lastX - prevX, lastY - prevY) < 0.005) points.pop();
  }
  closeDraft();
}

function startVertexDrag(event: PointerEvent, handle: Handle) {
  event.preventDefault();
  event.stopPropagation();
  if (handle.polygon < 0) {
    // 绘制中的顶点：点回第一个即闭合
    if (handle.vertex === 0 && draft.value && draft.value.points.length >= 3) closeDraft();
    return;
  }
  const region = currentParts.value[handle.part];
  const polygon = region?.polygons[handle.polygon];
  if (!polygon) return;
  pushHistory();
  selectedPart.value = handle.part;
  selectedVertex.value = { part: handle.part, polygon: handle.polygon, vertex: handle.vertex };

  const onMove = (moveEvent: PointerEvent) => {
    const point = toImagePoint(moveEvent.clientX, moveEvent.clientY);
    if (point) polygon[handle.vertex] = point;
  };
  const onUp = () => window.removeEventListener("pointermove", onMove);
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp, { once: true });
}

function deleteSelectedVertex() {
  const target = selectedVertex.value;
  if (!target) return;
  const polygon = currentParts.value[target.part]?.polygons[target.polygon];
  if (!polygon) return;
  pushHistory();
  if (polygon.length <= 3) {
    removePolygon(target.part, target.polygon);
  } else {
    polygon.splice(target.vertex, 1);
  }
  selectedVertex.value = null;
}

function onKeyDown(event: KeyboardEvent) {
  if (testMode.value) {
    if (event.key === "Escape") toggleTestMode();
    return;
  }
  const target = event.target as HTMLElement | null;
  if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
  if (event.key === "Escape") {
    if (draft.value) draft.value = null;
    else {
      selectedPart.value = null;
      selectedVertex.value = null;
    }
    return;
  }
  if (event.key === "Enter" && draft.value) {
    event.preventDefault();
    closeDraft();
    return;
  }
  if (event.key === "Delete" || event.key === "Backspace") {
    if (selectedVertex.value) deleteSelectedVertex();
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") {
    event.preventDefault();
    undo();
  }
}

async function close() {
  if (dirty.value) {
    const confirmed = await dialogStore.confirm(
      t("settings.characterInfo.touch.discardMessage"),
      t("settings.characterInfo.touch.discardTitle"),
    );
    if (!confirmed) return;
  }
  emit("close");
}

// 只应用到父弹窗草稿，由底部保存按钮统一提交。
function apply() {
  emit("apply", serializeBodyPart(costumes.value ?? {}));
}

let observer: ResizeObserver | null = null;
let imageToken = 0;

function measureBox() {
  const element = overlayRef.value;
  if (!element) return;
  const { width, height } = element.getBoundingClientRect();
  boxAspect.value = height > 0 ? width / height : 0;
}

async function measureImage(url: string) {
  const token = ++imageToken;
  imageAspect.value = 0;
  const aspect = await loadImageAspect(url);
  if (token !== imageToken || !aspect) return;
  imageAspect.value = aspect;
}

// 旧数据要靠取景换算，所以第一次拿到几何之后才解析，之后不再重解析，免得丢掉编辑
watch(
  () => frame.value,
  (value) => {
    if (!value || costumes.value) return;
    costumes.value = parseBodyPart(props.bodyPart, value).costumes;
  },
  { immediate: true },
);

watch(avatarUrl, measureImage, { immediate: true });

watch(
  overlayRef,
  (element, previous) => {
    if (previous) observer?.unobserve(previous);
    measureBox();
    if (element) observer?.observe(element);
  },
  { flush: "post" },
);

onMounted(() => {
  measureBox();
  observer = new ResizeObserver(() => {
    measureBox();
  });
  if (overlayRef.value) observer.observe(overlayRef.value);
  window.addEventListener("keydown", onKeyDown);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  window.removeEventListener("keydown", onKeyDown);
});
</script>

<style scoped>
.region-shape {
  fill: transparent;
  fill-opacity: 0.15;
  stroke-width: 2;
  stroke-dasharray: 6 4;
  cursor: pointer;
  pointer-events: all;
}

.region-shape-selected {
  fill: currentColor;
  fill-opacity: 0.3;
  stroke-width: 3;
}

.region-shape-testing {
  pointer-events: none;
}

.region-draft {
  fill: none;
  stroke: #fff;
  stroke-width: 2;
  stroke-dasharray: 4 4;
  pointer-events: none;
}

.region-handle {
  position: absolute;
  width: 12px;
  height: 12px;
  margin: -6px 0 0 -6px;
  border: 2px solid #fff;
  border-radius: 9999px;
  background: #5e72e4;
  cursor: grab;
  padding: 0;
}

.region-handle-selected {
  background: #fb6340;
}

.live2d-btn,
.live2d-btn-primary,
.live2d-icon-btn {
  cursor: pointer;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 0.5rem;
  padding: 0.25rem 0.6rem;
  font-size: 0.75rem;
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.live2d-btn:disabled,
.live2d-btn-primary:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.live2d-btn-primary {
  border-color: transparent;
  background: #5e72e4;
}

.live2d-icon-btn {
  padding: 0 0.35rem;
  line-height: 1.2;
}

.live2d-select {
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 0.5rem;
  background: rgba(0, 0, 0, 0.3);
  padding: 0.25rem 0.5rem;
  font-size: 0.75rem;
  color: #fff;
}
</style>
