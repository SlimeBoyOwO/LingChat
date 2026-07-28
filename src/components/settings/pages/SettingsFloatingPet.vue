<template>
  <MenuPage>
    <!-- 总开关 -->
    <MenuItem title="悬浮桌宠">
      <template #header>
        <Sparkles :size="20" />
      </template>

      <div class="flex items-center justify-between gap-4 mb-4">
        <div class="text-white/80 text-sm">
          启用后将在其它应用上层显示悬浮桌宠。仅 Android 端生效。
        </div>
        <label class="inline-flex items-center cursor-pointer">
          <input
            type="checkbox"
            class="sr-only peer"
            :checked="store.enabled"
            @change="onToggleEnabled"
          />
          <div
            class="w-11 h-6 bg-white/10 rounded-full peer peer-checked:bg-brand transition-all relative
                   after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:rounded-full
                   after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-5"
          ></div>
        </label>
      </div>

      <!-- 权限状态 -->
      <div
        class="flex items-center justify-between p-4 rounded-xl border border-white/10 bg-white/5 mb-4"
      >
        <div class="flex items-center gap-3">
          <ShieldCheck :size="18" :class="permissionIconClass" />
          <div>
            <div class="text-sm font-medium text-white/90">
              叠加层权限：{{ permissionLabel }}
            </div>
            <div class="text-xs text-white/50 mt-1">
              {{ permissionHint }}
            </div>
          </div>
        </div>
        <button
          v-if="store.permission !== 'granted' && store.permission !== 'unsupported'"
          class="px-4 py-1.5 rounded-full text-sm font-medium bg-brand/80 text-white hover:bg-brand"
          @click="requestPermission"
        >
          去开启
        </button>
      </div>

      <!-- 行为开关 -->
      <div class="flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-white/90">拖动后自动贴边</div>
            <div class="text-xs text-white/50 mt-1">松手后吸附到屏幕左右边缘</div>
          </div>
          <label class="inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              class="sr-only peer"
              :checked="settings.floatingPet.snapToEdge"
              @change="onToggleSnap"
            />
            <div
              class="w-11 h-6 bg-white/10 rounded-full peer peer-checked:bg-brand transition-all relative
                     after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:rounded-full
                     after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-5"
            ></div>
          </label>
        </div>

        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-white/90">启动后自动弹出</div>
            <div class="text-xs text-white/50 mt-1">App 启动且权限就绪时直接显示桌宠</div>
          </div>
          <label class="inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              class="sr-only peer"
              :checked="settings.floatingPet.autoShowOnLaunch"
              @change="onToggleAutoShow"
            />
            <div
              class="w-11 h-6 bg-white/10 rounded-full peer peer-checked:bg-brand transition-all relative
                     after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:rounded-full
                     after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-5"
            ></div>
          </label>
        </div>
      </div>
    </MenuItem>

    <!-- 桌宠控制 -->
    <MenuItem title="桌宠控制">
      <template #header>
        <CirclePlay :size="20" />
      </template>

      <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <button
          class="px-4 py-2.5 rounded-xl text-sm font-medium border transition-all
                 bg-brand/80 text-white border-brand hover:bg-brand disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!store.isSupported"
          @click="onShow"
        >
          显示桌宠
        </button>
        <button
          class="px-4 py-2.5 rounded-xl text-sm font-medium border transition-all
                 bg-white/10 text-white/80 border-white/20 hover:bg-white/20 disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!store.isSupported"
          @click="onHide"
        >
          隐藏
        </button>
        <button
          class="px-4 py-2.5 rounded-xl text-sm font-medium border transition-all
                 bg-red-500/20 text-red-300 border-red-500/30 hover:bg-red-500/30 disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!store.isSupported"
          @click="onStop"
        >
          完全停止服务
        </button>
      </div>

      <div class="mt-4 text-xs text-white/40 leading-5">
        双击桌宠头像 = 隐藏；长按 = 弹出操作菜单（仅运行中）。
      </div>
    </MenuItem>

    <!-- 外观 -->
    <MenuItem title="外观">
      <template #header>
        <Settings2 :size="20" />
      </template>

      <div>
        <div class="flex items-center justify-between mb-2">
          <div class="text-sm font-medium text-white/90">尺寸缩放</div>
          <div class="text-sm text-white/60">{{ scalePercent }}%</div>
        </div>
        <input
          type="range"
          min="50"
          max="200"
          step="5"
          :value="scaleValue"
          class="w-full accent-brand"
          @input="onScale"
        />
        <div class="flex justify-between text-xs text-white/40 mt-1">
          <span>50%</span>
          <span>100%</span>
          <span>200%</span>
        </div>
      </div>
    </MenuItem>

    <!-- 状态 / 调试 -->
    <MenuItem title="状态">
      <template #header>
        <Activity :size="20" />
      </template>

      <div class="grid grid-cols-2 gap-3 text-xs">
        <div class="p-3 rounded-lg bg-white/5 border border-white/10">
          <div class="text-white/50 mb-1">最近手势</div>
          <div class="text-white/90 font-mono">
            {{ store.lastEvent ? store.lastEvent.type : "-" }}
          </div>
        </div>
        <div class="p-3 rounded-lg bg-white/5 border border-white/10">
          <div class="text-white/50 mb-1">最近推送状态</div>
          <div class="text-white/90 font-mono truncate">
            {{
              store.lastPushed
                ? Object.keys(store.lastPushed).join(", ")
                : "-"
            }}
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Activity,
  CirclePlay,
  Settings2,
  ShieldCheck,
  Sparkles,
} from "lucide-vue-next";
import { MenuItem, MenuPage } from "@/components/ui";
import { useSettingsStore } from "@/stores/modules/settings";
import {
  requestOverlayPermission,
  startPermissionExplanation,
  stopFloatingPetService,
  showFloatingPet,
  hideFloatingPet,
} from "@/api/services/floating-pet";
import { useFloatingPetStore } from "@/stores/modules/floating-pet";
import { useUIStore } from "@/stores/modules/ui/ui";

const settings = useSettingsStore();
const store = useFloatingPetStore();
const uiStore = useUIStore();

const scaleValue = ref<number>(
  Math.round((settings.pet.scale ?? 1.0) * 100),
);

const scalePercent = computed(() => scaleValue.value);

const permissionLabel = computed(() => {
  switch (store.permission) {
    case "granted":
      return "已授权";
    case "denied":
      return "已拒绝";
    case "unsupported":
      return "当前平台不支持";
    default:
      return "未查询";
  }
});

const permissionHint = computed(() => {
  switch (store.permission) {
    case "granted":
      return "可以正常显示悬浮桌宠";
    case "denied":
      return "请到系统设置开启「显示在其它应用上层」";
    case "unsupported":
      return "仅 Android 端可用，桌面端请使用桌宠窗口模式";
    default:
      return "首次启用时会弹窗提示";
  }
});

const permissionIconClass = computed(() => {
  if (store.permission === "granted") return "text-green-400";
  if (store.permission === "denied") return "text-red-400";
  if (store.permission === "unsupported") return "text-white/40";
  return "text-yellow-400";
});

onMounted(async () => {
  await store.refreshPermission();
  if (store.permission === "unknown" && store.enabled) {
    await store.activate();
  }
});

async function onToggleEnabled(e: Event) {
  const v = (e.target as HTMLInputElement).checked;
  store.setEnabled(v);
  if (v && store.permission === "granted") {
    await showFloatingPet(scaleValue.value / 100);
  }
}

function onToggleSnap(e: Event) {
  settings.update("floatingPet.snapToEdge", (e.target as HTMLInputElement).checked);
}

function onToggleAutoShow(e: Event) {
  settings.update(
    "floatingPet.autoShowOnLaunch",
    (e.target as HTMLInputElement).checked,
  );
}

function onScale(e: Event) {
  const v = Number((e.target as HTMLInputElement).value);
  scaleValue.value = v;
  settings.update("pet.scale", v / 100);
}

async function requestPermission() {
  try {
    if (!store.explanationShown) {
      await startPermissionExplanation();
      store.explanationShown = true;
    }
    await requestOverlayPermission();
    // 重新查询（用户回前台后权限可能已变化）
    setTimeout(() => void store.refreshPermission(), 500);
  } catch (err) {
    uiStore.showNotification({
      type: "error",
      title: "请求权限失败",
      message: `${(err as Error).message ?? err}`,
      skipTipsCheck: true,
    });
  }
}

async function onShow() {
  const ok = await store.activate(scaleValue.value / 100);
  if (!ok) {
    uiStore.showNotification({
      type: "warning",
      title: "无法启动桌宠",
      message: "检查权限或总开关",
      skipTipsCheck: true,
    });
  }
}

async function onHide() {
  await store.deactivate();
}

async function onStop() {
  await store.stop();
  uiStore.showNotification({
    type: "success",
    title: "桌宠服务已停止",
    message: "悬浮桌宠 Service 已完全停止。",
    skipTipsCheck: true,
  });
}
</script>
