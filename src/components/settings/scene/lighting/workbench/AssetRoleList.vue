<script setup lang="ts">
/**
 * 素材管理：场景里可能出场的角色，每人一套专属光影。
 *
 * 只做「选谁 / 谁已配过 / 清掉某人的配置」三件事，参数编辑仍由中间画布与右栏承担。
 * 没配过的角色跟随场景默认那套，聊天侧也是这个口径。
 */ import type { RoleAsset } from "../types";

defineProps<{
  roles: RoleAsset[];
  /** 当前编辑目标；null = 场景默认那套 */
  activeRoleId: number | null;
  loading: boolean;
}>();

const emit = defineEmits<{
  (e: "pick", roleId: number | null): void;
  (e: "clear", roleId: number): void;
}>();
</script>

<template>
  <div class="asset-list">
    <div class="wb-section-head" style="padding-left: 11px">
      <span class="wb-section-title">素材</span>
      <span class="wb-section-count"
        >{{ roles.filter((r) => r.configured).length }} / {{ roles.length }}</span
      >
    </div>

    <div class="asset-scroll wb-scroll">
      <button
        type="button"
        class="asset-row asset-scene"
        :class="{ 'is-active': activeRoleId === null }"
        @click="emit('pick', null)"
      >
        <span class="asset-row-bar" />
        <span class="asset-thumb asset-thumb-scene">
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor">
            <path
              d="M2.4 12.2 6 7.8l2.6 3 2.2-2.4 2.8 3.8z"
              stroke-width="1.2"
              stroke-linejoin="round"
            />
            <circle cx="5.4" cy="4.8" r="1.4" stroke-width="1.2" />
          </svg>
        </span>
        <span class="asset-text">
          <span class="asset-name wb-ellipsis">场景默认</span>
          <span class="asset-sub wb-ellipsis">没单独配的角色都用这套</span>
        </span>
      </button>

      <div v-if="loading" class="asset-loading">正在读取角色立绘…</div>

      <button
        v-for="role in roles"
        :key="role.roleId"
        type="button"
        class="asset-row"
        :class="{ 'is-active': role.roleId === activeRoleId }"
        @click="emit('pick', role.roleId)"
      >
        <span class="asset-row-bar" />
        <span class="asset-thumb">
          <img v-if="role.preview" :src="role.preview" :alt="role.name" />
          <span v-else class="asset-thumb-empty">{{ role.name.slice(0, 1) }}</span>
        </span>
        <span class="asset-text">
          <span class="asset-name wb-ellipsis">{{ role.name }}</span>
          <span class="asset-sub wb-ellipsis">{{
            role.configured ? "已配专属光影" : "跟随场景默认"
          }}</span>
        </span>
        <span
          v-if="role.configured"
          class="asset-clear"
          title="清除专属光影，改回跟随场景默认"
          @click.stop="emit('clear', role.roleId)"
        >
          <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
            <path
              d="M4.4 4.4l7.2 7.2M11.6 4.4l-7.2 7.2"
              stroke-width="1.4"
              stroke-linecap="round"
            />
          </svg>
        </span>
      </button>
    </div>

    <p class="asset-note">多个角色同时在台时只给场景打光，专属光影只在单独对话时生效。</p>
  </div>
</template>

<style scoped>
.asset-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}

.asset-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 6px;
}

.asset-loading {
  padding: 6px 8px;
  font-size: 10.5px;
  color: var(--wb-muted);
}

.asset-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 5px 6px 5px 8px;
  border-radius: var(--wb-r-md);
  cursor: pointer;
  text-align: left;
  transition: background 0.12s ease;
}

.asset-row:hover {
  background: var(--wb-hover);
}

.asset-row.is-active {
  background: var(--wb-accent-weak);
}

.asset-row-bar {
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 2px;
  background: transparent;
}

.asset-row.is-active .asset-row-bar {
  background: var(--wb-accent);
}

.asset-thumb {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 38px;
  flex-shrink: 0;
  overflow: hidden;
  border-radius: 5px;
  border: 1px solid var(--wb-border);
  background: var(--wb-surface-2);
  color: var(--wb-muted);
}

.asset-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top center;
}

.asset-thumb-empty {
  font-size: 12px;
  font-weight: 600;
}

.asset-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex: 1;
  min-width: 0;
}

.asset-name {
  font-size: 11.5px;
  color: var(--wb-text-2);
}

.asset-row.is-active .asset-name {
  color: var(--wb-accent);
  font-weight: 600;
}

.asset-sub {
  font-size: 9.5px;
  color: var(--wb-muted);
}

.asset-clear {
  display: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  border-radius: 4px;
  color: var(--wb-muted);
}

.asset-row:hover .asset-clear {
  display: flex;
}

.asset-clear:hover {
  background: #fdecec;
  color: var(--wb-danger);
}

.asset-note {
  flex-shrink: 0;
  padding: 7px 11px;
  border-top: 1px solid var(--wb-border);
  font-size: 9.5px;
  line-height: 1.5;
  color: var(--wb-muted);
}
</style>
