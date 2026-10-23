<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="visible"
        class="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
        @click="handleClose"
      >
        <div
          class="flex h-[85dvh] w-full max-w-3xl flex-col overflow-hidden rounded-3xl border border-white/20 bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] text-white shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] backdrop-blur-[30px] backdrop-saturate-180"
          @click.stop
        >
          <!-- Header（与角色设置弹窗同款外壳，去掉左侧 tab） -->
          <div
            class="flex items-center justify-between border-b border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)] p-6"
          >
            <div class="flex items-center gap-4">
              <div
                class="flex h-12 w-12 items-center justify-center rounded-xl bg-white/10 shadow-inner"
              >
                <Icon icon="mic" />
              </div>
              <div>
                <h2 class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                  {{ $t("settings.asr.globalHotword.dialog.title") }}
                </h2>
                <p class="m-0 text-sm text-white/50">
                  {{ $t("settings.asr.globalHotword.dialog.subtitle") }}
                </p>
              </div>
            </div>
            <button
              class="flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border-none bg-white/10 text-white transition-all duration-200 hover:rotate-90 hover:bg-white/20"
              @click="handleClose"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <!-- Content -->
          <div class="relative flex-1 overflow-y-auto p-6">
            <div v-if="loading" class="flex h-full items-center justify-center">
              <div
                class="h-10 w-10 animate-spin rounded-full border-3 border-white/10 border-t-[#5e72e4]"
              ></div>
            </div>
            <div v-else class="mx-auto max-w-3xl">
              <!-- `role-id=0` 仅作新建条目的占位，不入库、不上行 -->
              <HotwordSettings
                v-model="draft"
                :role-id="0"
                :model-capability="modelCapability"
                scope="global"
                :save-hotwords="saveHotwordRows"
              />
            </div>
          </div>

          <!-- Footer：无危险区，只有取消 / 保存 -->
          <div
            class="flex items-center justify-between gap-3 border-t border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)] p-4"
          >
            <span class="text-sm text-white/40">
              {{ $t("settings.asr.globalHotword.dialog.footerHint") }}
            </span>
            <div class="flex gap-3">
              <button
                class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
                @click="handleClose"
              >
                {{ $t("settings.asr.globalHotword.dialog.footer.cancel") }}
              </button>
              <button
                class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)] disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="saving || loading"
                @click="save"
              >
                <span
                  v-if="saving"
                  class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"
                ></span>
                {{
                  saving
                    ? $t("settings.asr.globalHotword.dialog.footer.saving")
                    : $t("settings.asr.globalHotword.dialog.footer.save")
                }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import {
  getGlobalHotwords,
  saveGlobalHotwords,
  type RoleHotword,
  type RoleHotwordDraft,
} from "@/api/services/hotword";
import { Icon } from "@/components/base";
import HotwordSettings from "@/components/settings/character/HotwordSettings.vue";
import { refreshSceneHotwords } from "@/composables/asr/hotwords";
import { useAsrModelCapability } from "@/composables/asr/modelCapability";
import { useDialogStore } from "@/stores/modules/ui/dialog";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const dialogStore = useDialogStore();
const modelCapability = useAsrModelCapability();

const loading = ref(false);
const saving = ref(false);
/** 草稿：编辑只进这里，提交由底部「保存」负责，「取消」即丢弃（与角色热词同款）。 */
const draft = ref<RoleHotwordDraft>({ hotwords: [] });

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    loading.value = true;
    try {
      // 保存是整表替换，读失败必须让弹窗打不开——草稿默认是空表，若放行，
      // 用户随手点一次「保存」就会静默清空全局热词
      const hotwords = await getGlobalHotwords();
      draft.value = { hotwords };
    } catch (e) {
      console.error("[GlobalHotwordDialog] 读取全局热词失败:", e);
      await dialogStore.alert(t("settings.asr.globalHotword.dialog.loadFailed"));
      emit("close");
    } finally {
      loading.value = false;
    }
  },
);

const handleClose = () => emit("close");

/** 批量导入保存：只上行 {text, weight}；成功后刷新运行时缓存。 */
const saveHotwordRows = async (rows: RoleHotword[]) => {
  await saveGlobalHotwords(rows.map(({ text, weight }) => ({ text, weight })));
  void refreshSceneHotwords();
};

const save = async () => {
  saving.value = true;
  try {
    // 只上行 {text, weight}：role_id 是组件内的占位字段
    await saveGlobalHotwords(draft.value.hotwords.map(({ text, weight }) => ({ text, weight })));
    // 运行时缓存与库同步（识别时注入的就是这份）：保存成功后重取
    void refreshSceneHotwords();
    emit("close");
  } catch (e) {
    console.error("[GlobalHotwordDialog] 保存全局热词失败:", e);
    await dialogStore.alert(t("settings.asr.globalHotword.dialog.saveFailed"));
  } finally {
    saving.value = false;
  }
};
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(10px);
}
</style>
