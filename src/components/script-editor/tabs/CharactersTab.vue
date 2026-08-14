<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { Button, Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

const emit = defineEmits<{
  'new-character': []
  'import-character': []
}>()

const { t } = useI18n()
const store = useScriptEditorStore()

/** 绝对路径 → webview 能加载的 asset URL，与 GameBackground / GameRoleAvatar 同一套 */
const assetUrl = (path: string) => convertFileSrc(path)
</script>

<template>
  <MenuPage>
    <MenuItem :title="t('scriptEditor.characters.menuTitle')">
      <template #header>
        <Icon
          icon="character"
          :size="20"
        />
      </template>

      <p
        class="mb-[0.9rem]
          rounded-xl
          border
          border-white/10
          bg-black/16
          px-[0.85rem]
          py-[0.7rem]
          text-[0.76rem]
          leading-[1.85]
          text-white/60
          [&_b]:font-semibold
          [&_b]:text-white/85
          [&_code]:font-mono
          [&_code]:text-brand"
        v-html="t('scriptEditor.characters.intro')"
      ></p>

      <p
        v-if="store.characters.length === 0"
        class="py-8
          text-center
          text-[0.85rem]
          text-white/45"
      >
        {{ t('scriptEditor.characters.empty') }}
      </p>
      <div
        v-for="c in store.characters"
        :key="c.folder"
        class="w-full
          border
          border-white/10
          rounded-[10px]
          px-[13px]
          py-[11px]
          mb-2
          bg-white/6
          transition-all
          duration-200
          flex
          items-center
          group"
      >
        <!-- 立绘缩略图：本地 avatar 优先，没有回退全局；都没有时占位，与
             引擎运行时同一个查找顺序，避免「编辑器看着有、游戏里没有」 -->
        <div
          class="char-thumb
            shrink-0
            w-11
            h-11
            rounded-full
            overflow-hidden
            border-[1.5px]
            border-brand/35"
        >
          <img
            v-if="c.previewImage"
            :src="assetUrl(c.previewImage)"
            :alt="c.aiName"
            class="w-full
              h-full
              object-cover
              object-[top_center]"
            loading="lazy"
          />
          <span
            v-else
            class="flex
              items-center
              justify-center
              w-full
              h-full
              text-[0.56rem]
              text-white/35"
            >{{ t('scriptEditor.characters.noPortrait') }}</span
          >
        </div>
        <div class="flex
          min-w-0
          flex-1
          flex-col
          gap-0.5">
          <div class="flex
            items-baseline
            gap-2">
            <span class="font-semibold
              text-white">{{ c.aiName }}</span>
            <code class="font-mono
              text-brand">character: {{ c.roleKey }}</code>
            <span
              v-if="c.emotions.length === 0 && c.globalAvatar"
              class="shrink-0
                border
                border-brand/40
                rounded-full
                px-[7px]
                py-px
                text-[0.6rem]
                text-brand
                bg-brand/12"
              :title="t('scriptEditor.characters.noLocalAvatar')"
              >{{ t('scriptEditor.characters.usesGlobalAvatar') }}</span
            >
            <span class="ml-auto
              text-xs
              text-white/40">
              {{ t('scriptEditor.characters.emotions', { count: c.emotions.length })
              }}{{
                c.clothes.length
                  ? t('scriptEditor.characters.clothes', { count: c.clothes.length })
                  : ''
              }}
            </span>
          </div>
          <p
            v-if="!c.previewImage"
            class="mt-1
              text-xs
              text-yellow-200"
          >
            {{ t('scriptEditor.characters.noPortraitWarn') }}
          </p>
          <p
            v-else
            class="mt-1
              text-xs
              text-white/40"
          >
            {{ c.emotions.slice(0, 12).join(t('scriptEditor.characters.emotionSep'))
            }}{{ c.emotions.length > 12 ? ' …' : '' }}
          </p>
        </div>
        <button
          class="shrink-0
            rounded
            px-[5px]
            text-[11px]
            text-white/25
            opacity-0
            transition-all
            duration-150
            group-hover:opacity-100
            hover:text-red-300
            hover:bg-red-400/15"
          :title="t('scriptEditor.characters.delete')"
          @click="store.deleteCharacter(c.folder, c.aiName)"
        >
          ✕
        </button>
      </div>

      <div class="mt-4
        flex
        flex-wrap
        gap-2">
        <Button
          type="big"
          @click="emit('new-character')"
        >
          {{ t('scriptEditor.characters.newCharacter') }}
        </Button>
        <Button
          type="big"
          @click="emit('import-character')"
        >
          {{ t('scriptEditor.characters.importCharacter') }}
        </Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<style scoped>
/* 棋盘底纹：透明图片不至于糊成一片黑 */
.char-thumb {
  background: repeating-conic-gradient(rgba(255, 255, 255, 0.08) 0% 25%, transparent 0% 50%) 0 0 /
    10px 10px;
}
</style>
