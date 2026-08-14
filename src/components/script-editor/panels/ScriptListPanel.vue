<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Button, Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

const emit = defineEmits<{ 'new-script': [] }>()

const { t } = useI18n()
const store = useScriptEditorStore()
</script>

<template>
  <MenuPage>
    <MenuItem :title="t('scriptEditor.scriptList.title')">
      <template #header>
        <Icon
          icon="package"
          :size="20"
        />
      </template>

      <p
        v-if="store.loading"
        class="py-8
          text-center
          text-[0.85rem]
          text-white/45"
      >
        {{ t('scriptEditor.scriptList.loading') }}
      </p>
      <p
        v-else-if="store.scripts.length === 0"
        class="py-8
          text-center
          text-[0.85rem]
          text-white/45"
      >
        {{ t('scriptEditor.scriptList.empty') }}
      </p>

      <div
        v-for="s in store.scripts"
        :key="s.key"
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
          cursor-pointer
          hover:border-brand
          hover:bg-[rgba(121,217,255,0.08)]
          group"
        @click="store.openScript(s.key)"
      >
        <div class="flex
          items-baseline
          gap-2">
          <span class="font-semibold
            text-white">{{ s.scriptName }}</span>
          <span
            v-if="s.isAdventure"
            class="border
              border-brand/35
              rounded-full
              px-[7px]
              text-[0.62rem]
              text-brand
              bg-brand/12"
            >{{ t('scriptEditor.scriptList.adventure') }}</span
          >
          <span
            v-if="!s.loadedByEngine"
            class="border
              border-amber-300/35
              rounded-full
              px-[7px]
              text-[0.62rem]
              text-amber-300
              bg-amber-300/12"
            >{{ t('scriptEditor.scriptList.notLoaded') }}</span
          >
          <span class="ml-auto
            text-xs
            text-white/40">{{
            t('scriptEditor.scriptList.chapters', { count: s.chapterCount })
          }}</span>
          <button
            class="rounded
              px-[5px]
              text-[11px]
              leading-[1.4]
              text-white/25
              opacity-0
              transition-all
              duration-150
              group-hover:opacity-100
              hover:text-red-300
              hover:bg-red-400/15"
            :title="t('scriptEditor.scriptList.delete')"
            @click.stop="store.deleteScript(s.key, s.scriptName)"
          >
            ✕
          </button>
        </div>
        <p class="mt-1
          text-xs
          text-white/50">
          {{ s.description || t('scriptEditor.scriptList.noDescription') }}
        </p>
        <p class="mt-1
          font-mono
          text-[10px]
          text-white/25">{{ s.key }}</p>
      </div>

      <Button
        type="big"
        class="mt-4"
        @click="emit('new-script')"
      >
        {{ t('scriptEditor.scriptList.newScript') }}
      </Button>
    </MenuItem>
  </MenuPage>
</template>
