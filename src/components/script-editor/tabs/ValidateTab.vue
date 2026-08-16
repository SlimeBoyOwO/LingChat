<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { Diagnostic } from '@/api/services/script-editor'

const { t } = useI18n()
const store = useScriptEditorStore()

const diagnosticsOf = (chapterId: string) =>
  (store.report?.diagnostics ?? []).filter((d) => d.chapter === chapterId)

const chapterHas = (chapterId: string) => diagnosticsOf(chapterId).length > 0

const jumpTo = async (d: Diagnostic) => {
  if (!d.chapter) {
    store.tab = 'config'
    return
  }
  store.tab = 'flow'
  if (store.chapter?.id !== d.chapter) {
    // openChapter 可能失败（读盘出错），失败时不要把 selectedEvent 设成别的章节的下标
    if (!(await store.openChapter(d.chapter))) return
  } else {
    store.level = 'chapter'
  }
  if (d.eventIndex !== undefined) store.selectedEvent = d.eventIndex
}

/** 章节头部的「打开」：与 jumpTo 一样要先切到流程页，否则打开结果看不到 */
const openChapterFromValidate = async (chapterId: string) => {
  store.tab = 'flow'
  if (store.chapter?.id !== chapterId) {
    await store.openChapter(chapterId)
  } else {
    store.level = 'chapter'
  }
}
</script>

<template>
  <MenuPage>
    <MenuItem :title="t('scriptEditor.validate.menuTitle')">
      <template #header>
        <Icon
          icon="achievement"
          :size="20"
        />
      </template>

      <div class="flex
        flex-wrap
        items-center
        gap-2
        mb-3">
        <button
          class="inline-flex
            items-center
            gap-1
            border
            border-white/10
            rounded-lg
            px-3
            py-[0.3rem]
            text-[0.8rem]
            whitespace-nowrap
            text-white/70
            bg-white/6
            transition-all
            duration-200
            hover:enabled:text-white
            hover:enabled:bg-white/[0.12]
            disabled:cursor-not-allowed
            disabled:opacity-40"
          @click="store.runValidation()"
        >
          {{ t('scriptEditor.validate.revalidate') }}
        </button>
        <span
          v-if="store.report"
          class="text-[0.78rem]
            text-white/50
            [&_b]:font-semibold"
        >
          <b class="text-red-300">{{ store.report.errorCount }}</b>
          {{ t('scriptEditor.validate.errors') }} ·
          <b class="text-amber-300">{{ store.report.warnCount }}</b>
          {{ t('scriptEditor.validate.warns') }} ·
          <b class="text-white/50">{{ store.report.infoCount }}</b>
          {{ t('scriptEditor.validate.infos') }}
        </span>
      </div>

      <p
        v-if="!store.report"
        class="py-8
          text-center
          text-[0.85rem]
          text-white/45"
      >
        {{ t('scriptEditor.validate.checking') }}
      </p>
      <p
        v-else-if="store.report.diagnostics.length === 0"
        class="rounded-xl
          border
          border-green-400/30
          bg-green-400/10
          px-[0.9rem]
          py-[0.9rem]
          text-[0.82rem]
          text-green-300"
      >
        {{ t('scriptEditor.validate.clean') }}
      </p>

      <template v-else>
        <!-- 剧本级问题 -->
        <div
          v-if="store.scriptDiagnostics.length"
          class="mb-3
            rounded-[10px]
            border
            border-white/10
            bg-black/15
            overflow-hidden"
        >
          <div
            class="flex
              items-center
              gap-[0.6rem]
              border-b
              border-white/[0.07]
              px-[0.8rem]
              py-[0.55rem]"
          >
            <span class="text-[0.82rem]
              font-semibold
              text-white">{{
              t('scriptEditor.validate.scriptLevel')
            }}</span>
            <span class="font-mono
              text-[0.66rem]
              text-white/30">story_config.yaml</span>
          </div>
          <div
            v-for="(d, i) in store.scriptDiagnostics"
            :key="i"
            class="flex
              items-start
              gap-2
              px-[0.8rem]
              py-[0.45rem]
              text-[0.76rem]
              leading-[1.75]
              text-white/75"
          >
            <span
              class="shrink-0
                w-1.5
                h-1.5
                mt-[0.55rem]
                rounded-full"
              :class="{
                'bg-red-400': d.severity === 'error',
                'bg-amber-400': d.severity === 'warn',
                'bg-white/30': d.severity === 'info',
              }"
            ></span>
            <span class="flex-1">{{ d.message }}</span>
          </div>
        </div>

        <!-- 按章节聚合，与流程图同样的顺序 -->
        <div
          v-for="c in store.chapters"
          :key="c.id"
          class="mb-3
            rounded-[10px]
            border
            border-white/10
            bg-black/15
            overflow-hidden"
          :class="{ 'opacity-55': !chapterHas(c.id) }"
        >
          <div
            class="flex
              items-center
              gap-[0.6rem]
              border-b
              border-white/[0.07]
              px-[0.8rem]
              py-[0.55rem]"
          >
            <span class="text-[0.82rem]
              font-semibold
              text-white">{{ c.name || c.id }}</span>
            <span class="font-mono
              text-[0.66rem]
              text-white/30">{{ c.id }}.yaml</span>
            <span class="flex
              gap-[0.6rem]
              ml-auto
              text-[0.7rem]
              [&_b]:font-semibold">
              <b
                v-if="store.diagnosticsByChapter[c.id]?.errors"
                class="text-red-300"
                >{{ store.diagnosticsByChapter[c.id].errors }}
                {{ t('scriptEditor.validate.errors') }}</b
              >
              <b
                v-if="store.diagnosticsByChapter[c.id]?.warns"
                class="text-amber-300"
                >{{ store.diagnosticsByChapter[c.id].warns }}
                {{ t('scriptEditor.validate.warns') }}</b
              >
              <b
                v-if="store.diagnosticsByChapter[c.id]?.infos"
                class="text-white/50"
                >{{ store.diagnosticsByChapter[c.id].infos }}
                {{ t('scriptEditor.validate.infos') }}</b
              >
              <span
                v-if="!chapterHas(c.id)"
                class="text-green-300"
                >{{ t('scriptEditor.validate.passed') }}</span
              >
            </span>
            <button
              class="inline-flex
                items-center
                gap-1
                border
                border-white/10
                rounded-lg
                px-3
                py-[0.3rem]
                text-[0.8rem]
                whitespace-nowrap
                text-white/70
                bg-white/6
                transition-all
                duration-200
                hover:enabled:text-white
                hover:enabled:bg-white/[0.12]
                disabled:cursor-not-allowed
                disabled:opacity-40"
              @click="openChapterFromValidate(c.id)"
            >
              {{ t('scriptEditor.validate.open') }}
            </button>
          </div>

          <div
            v-for="(d, i) in diagnosticsOf(c.id)"
            :key="i"
            class="flex
              items-start
              gap-2
              px-[0.8rem]
              py-[0.45rem]
              text-[0.76rem]
              leading-[1.75]
              text-white/75
              cursor-pointer
              hover:bg-white/5"
            @click="jumpTo(d)"
          >
            <span
              class="shrink-0
                w-1.5
                h-1.5
                mt-[0.55rem]
                rounded-full"
              :class="{
                'bg-red-400': d.severity === 'error',
                'bg-amber-400': d.severity === 'warn',
                'bg-white/30': d.severity === 'info',
              }"
            ></span>
            <span class="flex-1">{{ d.message }}</span>
            <span
              v-if="d.eventIndex !== undefined"
              class="shrink-0
                text-[0.68rem]
                whitespace-nowrap
                text-brand
                opacity-70"
              >{{ t('scriptEditor.validate.eventJump', { index: d.eventIndex + 1 }) }} →</span
            >
          </div>
        </div>
      </template>
    </MenuItem>
  </MenuPage>
</template>
