<script setup lang="ts">
/**
 * 灯光列表：单行条目，右侧是强度值与显隐。
 *
 * 行内操作按稿的微交互：默认只显示「强度 + 眼睛」，悬停才浮出复制/删除，
 * 选中行左侧挂紫条。这样 8 盏灯也不会把左栏撑爆。
 */
import type { LightSource, LightType } from "@/api/services/scene";

const props = defineProps<{
  lights: LightSource[];
  selectedIndex: number;
  max?: number;
}>();

const emit = defineEmits<{
  (e: "select", index: number): void;
  (e: "add", type: LightType): void;
  (e: "remove", index: number): void;
  (e: "duplicate", index: number): void;
  (e: "toggle-enabled", index: number): void;
  (e: "toggle-key", index: number): void;
  (e: "move", index: number, dir: "up" | "down"): void;
}>();

const TYPE_LABEL: Record<LightType, string> = {
  point: "点光",
  ambient: "环境光",
  directional: "平行光",
  spot: "锥光",
};

function typeLabel(t: LightType): string {
  return TYPE_LABEL[t] ?? "光源";
}

function shortName(l: LightSource, idx: number): string {
  const name = (l.name || "").trim();
  if (!name) return `${typeLabel(l.type)} ${idx + 1}`;
  // 「点光源 #2」这类默认名压成「点光 2」，稿上一行放得下
  return name.replace(/^([^\s#]+)\s*#?\s*(\d+)?$/, (_m, base: string, num?: string) => {
    const label = TYPE_LABEL[l.type];
    return num ? `${label} ${num}` : label;
  });
}

function isFull(): boolean {
  return props.lights.length >= (props.max ?? 8);
}
</script>

<template>
  <div class="light-list">
    <div class="wb-section-head" style="padding-left: 11px">
      <span class="wb-section-title">灯光</span>
      <span class="wb-section-count">{{ lights.length }} / {{ max ?? 8 }}</span>
      <span class="wb-section-tail">
        <button
          type="button"
          class="wb-icon-btn"
          :disabled="isFull()"
          :title="isFull() ? '最多 8 盏' : '添加光源'"
          @click="emit('add', 'point')"
        >
          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
            <path d="M8 3.5v9M3.5 8h9" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        </button>
      </span>
    </div>

    <div class="light-scroll wb-scroll">
      <div
        v-for="(l, idx) in lights"
        :key="l.id"
        class="light-row"
        :class="{ 'is-active': idx === selectedIndex, 'is-off': !l.enabled }"
        @click="emit('select', idx)"
      >
        <span class="light-row-bar" />
        <span class="light-row-icon" :style="{ color: l.color }">
          <svg v-if="l.type === 'ambient'" viewBox="0 0 16 16" width="12" height="12">
            <circle cx="8" cy="8" r="5.2" fill="none" stroke="currentColor" stroke-width="1.3" />
            <circle cx="8" cy="8" r="2" fill="currentColor" opacity="0.6" />
          </svg>
          <svg v-else-if="l.type === 'directional'" viewBox="0 0 16 16" width="12" height="12">
            <circle cx="8" cy="8" r="2.6" fill="currentColor" />
            <g stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
              <path d="M8 1.6v1.8M8 12.6v1.8M1.6 8h1.8M12.6 8h1.8" />
              <path d="M3.6 3.6l1.3 1.3M11.1 11.1l1.3 1.3M12.4 3.6l-1.3 1.3M4.9 11.1l-1.3 1.3" />
            </g>
          </svg>
          <svg v-else-if="l.type === 'spot'" viewBox="0 0 16 16" width="12" height="12">
            <path d="M8 2.6 12.4 13H3.6L8 2.6Z" fill="currentColor" opacity="0.85" />
          </svg>
          <svg v-else viewBox="0 0 16 16" width="12" height="12">
            <circle cx="8" cy="8" r="3.4" fill="currentColor" />
            <circle cx="8" cy="8" r="5.6" fill="none" stroke="currentColor" opacity="0.35" />
          </svg>
        </span>

        <span class="light-row-name wb-ellipsis">{{ shortName(l, idx) }}</span>
        <!-- 主光入口：角色受光方向以它为准。之前这里只是个只读徽标，
             toggle-key 声明了却从不 emit，用户没有任何指定主光的手段 -->
        <button
          type="button"
          class="light-row-key"
          :class="{ 'is-key': l.is_key }"
          :title="l.is_key ? '取消主光' : '设为主光'"
          @click.stop="emit('toggle-key', idx)"
        >
          主
        </button>

        <span class="light-row-value">{{ l.intensity.toFixed(2) }}</span>

        <span class="light-row-actions">
          <button
            type="button"
            class="wb-icon-btn light-act"
            title="复制"
            @click.stop="emit('duplicate', idx)"
          >
            <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
              <rect x="5.5" y="5.5" width="7" height="7" rx="1.4" stroke-width="1.2" />
              <path d="M10.5 3.5H4.4A1.4 1.4 0 0 0 3 4.9v6.1" stroke-width="1.2" />
            </svg>
          </button>
          <button
            type="button"
            class="wb-icon-btn light-act"
            title="删除"
            :disabled="lights.length <= 1"
            @click.stop="emit('remove', idx)"
          >
            <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
              <path d="M4 6h8l-.7 7H4.7L4 6Z" stroke-width="1.2" />
              <path d="M6.5 6V4.2h3V6M3 6h10" stroke-width="1.2" stroke-linecap="round" />
            </svg>
          </button>
        </span>

        <button
          type="button"
          class="wb-icon-btn light-eye"
          :title="l.enabled ? '隐藏这盏灯' : '启用这盏灯'"
          @click.stop="emit('toggle-enabled', idx)"
        >
          <svg
            viewBox="0 0 16 16"
            width="12"
            height="12"
            fill="none"
            stroke="currentColor"
            stroke-width="1.2"
          >
            <path d="M1.8 8S4.2 4.2 8 4.2 14.2 8 14.2 8 11.8 11.8 8 11.8 1.8 8 1.8 8Z" />
            <circle cx="8" cy="8" r="1.9" />
            <path v-if="!l.enabled" d="M2.6 13.4 13.4 2.6" stroke-linecap="round" />
          </svg>
        </button>
      </div>

      <p v-if="!lights.length" class="wb-empty">还没有光源，点右上角「＋」添加</p>
    </div>

    <div class="light-add-row">
      <button
        type="button"
        class="wb-btn wb-btn-ghost wb-btn-block"
        @click="emit('add', 'ambient')"
      >
        ＋ 环境光
      </button>
      <button type="button" class="wb-btn wb-btn-ghost wb-btn-block" @click="emit('add', 'point')">
        ＋ 点光
      </button>
      <button
        type="button"
        class="wb-btn wb-btn-ghost wb-btn-block"
        @click="emit('add', 'directional')"
      >
        ＋ 平行光
      </button>
      <button type="button" class="wb-btn wb-btn-ghost wb-btn-block" @click="emit('add', 'spot')">
        ＋ 锥光
      </button>
    </div>
  </div>
</template>

<style scoped>
.light-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-top: 1px solid var(--wb-border);
}

.light-scroll {
  min-height: 0;
  overflow-y: auto;
  padding: 0 6px;
}

.light-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 27px;
  padding: 0 4px 0 8px;
  border-radius: var(--wb-r-md);
  cursor: pointer;
  transition: background 0.12s ease;
}

.light-row:hover {
  background: var(--wb-hover);
}

.light-row.is-active {
  background: var(--wb-accent-weak);
}

.light-row.is-off .light-row-name,
.light-row.is-off .light-row-value {
  opacity: 0.42;
}

.light-row-bar {
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 2px;
  border-radius: 2px;
  background: transparent;
}

.light-row.is-active .light-row-bar {
  background: var(--wb-accent);
}

.light-row-icon {
  display: flex;
  flex-shrink: 0;
  opacity: 0.9;
}

.light-row-name {
  flex: 1;
  min-width: 0;
  font-size: 11.5px;
  color: var(--wb-text-2);
}

.light-row.is-active .light-row-name {
  color: var(--wb-accent);
  font-weight: 600;
}

.light-row-key {
  flex-shrink: 0;
  font-size: 9px;
  line-height: 1;
  padding: 1.5px 3px;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  background: transparent;
  color: var(--wb-muted);
}

.light-row-key:hover {
  background: var(--wb-hover);
  color: var(--wb-text-2);
}

.light-row-key.is-key {
  background: var(--wb-accent-weak);
  color: var(--wb-accent);
}

.light-row-value {
  font-size: 11px;
  color: var(--wb-text);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.light-row-actions {
  display: none;
  align-items: center;
  gap: 0;
}

.light-row:hover .light-row-actions {
  display: flex;
}

.light-row:hover .light-row-value {
  display: none;
}

.light-act {
  width: 18px;
  height: 18px;
}

.light-eye {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.light-add-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2px;
  padding: 4px 6px 6px;
}

.light-add-row .wb-btn {
  height: 22px;
  font-size: 10.5px;
  color: var(--wb-muted);
}
</style>
