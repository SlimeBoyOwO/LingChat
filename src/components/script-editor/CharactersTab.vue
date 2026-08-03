<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { Button, Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

const emit = defineEmits<{
  'new-character': []
  'import-character': []
}>()

const store = useScriptEditorStore()

/** 绝对路径 → webview 能加载的 asset URL，与 GameBackground / GameRoleAvatar 同一套 */
const assetUrl = (path: string) => convertFileSrc(path)
</script>

<template>
  <MenuPage>
    <MenuItem title="剧本内角色">
      <template #header>
        <Icon
          icon="character"
          :size="20"
        />
      </template>

      <p class="mb-[0.9rem] rounded-xl border border-white/10 bg-black/16 px-[0.85rem] py-[0.7rem] text-[0.76rem] leading-[1.85] text-white/60">
        这里管理<b class="font-semibold text-white/85">只有这个剧本用得上的角色</b>。剧本里用
        <code class="font-mono text-brand">character: &lt;引用名&gt;</code> 指代；写
        <code class="font-mono text-brand">MAIN</code> 表示当前主角（羁绊剧本里就是绑定的那位，不需要导入，
        引擎直接从全局角色库读取）。
        <br />想用全局角色库里已有的人设，点下方「从全局角色库导入」复制一份到本剧本；
        立绘仍读全局那份，不会让剧本目录白白变大。
      </p>

      <p
        v-if="store.characters.length === 0"
        class="py-8 text-center text-[0.85rem] text-white/45"
      >
        还没有剧本内角色
      </p>
      <div
        v-for="c in store.characters"
        :key="c.folder"
        class="w-full border border-white/10 rounded-[10px] px-[13px] py-[11px] mb-2 bg-white/6 transition-all duration-200 flex items-center group"
      >
        <!-- 立绘缩略图：本地 avatar 优先，没有回退全局；都没有时占位，与
             引擎运行时同一个查找顺序，避免「编辑器看着有、游戏里没有」 -->
        <div class="char-thumb shrink-0 w-11 h-11 rounded-full overflow-hidden border-[1.5px] border-brand/35">
          <img
            v-if="c.previewImage"
            :src="assetUrl(c.previewImage)"
            :alt="c.aiName"
            class="w-full h-full object-cover object-[top_center]"
            loading="lazy"
          />
          <span
            v-else
            class="flex items-center justify-center w-full h-full text-[0.56rem] text-white/35"
            >无立绘</span>
        </div>
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
          <div class="flex items-baseline gap-2">
            <span class="font-semibold text-white">{{ c.aiName }}</span>
            <code class="font-mono text-brand">character: {{ c.roleKey }}</code>
            <span
              v-if="c.emotions.length === 0 && c.globalAvatar"
              class="shrink-0 border border-brand/40 rounded-full px-[7px] py-px text-[0.6rem] text-brand bg-brand/12"
              title="本剧本没复制立绘，但全局角色库里有；引擎会自动用全局那份"
              >立绘读自全局</span
            >
            <span class="ml-auto text-xs text-white/40">
              {{ c.emotions.length }} 个表情{{
                c.clothes.length ? ` · ${c.clothes.length} 套服装` : ''
              }}
            </span>
          </div>
          <p
            v-if="!c.previewImage"
            class="mt-1 text-xs text-yellow-200"
          >
            本剧本与全局角色库都没有这个角色的立绘，台词里它不会显示
          </p>
          <p
            v-else
            class="mt-1 text-xs text-white/40"
          >
            {{ c.emotions.slice(0, 12).join('、') }}{{ c.emotions.length > 12 ? ' …' : '' }}
          </p>
        </div>
        <button
          class="shrink-0 rounded px-[5px] text-[11px] text-white/25 opacity-0 transition-all duration-150 group-hover:opacity-100 hover:text-red-300 hover:bg-red-400/15"
          title="删除角色（移到 .trash/）"
          @click="store.deleteCharacter(c.folder, c.aiName)"
        >
          ✕
        </button>
      </div>

      <div class="mt-4 flex flex-wrap gap-2">
        <Button
          type="big"
          @click="emit('new-character')"
        >
          ＋ 新建角色
        </Button>
        <Button
          type="big"
          @click="emit('import-character')"
        >
          ↓ 从全局角色库导入
        </Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<style scoped>
/* 棋盘底纹：透明图片不至于糊成一片黑 */
.char-thumb {
  background:
    repeating-conic-gradient(rgba(255, 255, 255, 0.08) 0% 25%, transparent 0% 50%) 0 0 / 10px 10px;
}
</style>
