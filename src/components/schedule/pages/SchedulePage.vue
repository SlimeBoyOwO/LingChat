<template>
  <!-- 视图：日程主题列表 -->
  <div v-if="uiStore.scheduleView === 'schedule_groups'">
    <div v-if="Object.keys(scheduleGroups).length === 0" class="py-20 text-center text-slate-300">
      <Inbox class="mx-auto mb-4 h-10 w-10 opacity-20" />
      <p>{{ $t("ui.schedulePage.emptyGroups") }}</p>
    </div>
    <div v-else class="grid grid-cols-1 gap-6 sm:grid-cols-1 lg:grid-cols-2">
      <div
        v-for="(group, id) in scheduleGroups"
        :key="id"
        @click="selectGroup(id)"
        class="group glass-effect border-brand relative cursor-pointer rounded-3xl border p-6
          shadow-sm transition-all hover:-translate-y-1 hover:shadow-xl"
      >
        <button
          @click.stop="removeScheduleGroup(id)"
          class="absolute top-4 right-4 z-10 p-1 text-slate-300 hover:text-red-400"
        >
          <Trash2 :size="18" />
        </button>
        <div
          class="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-cyan-500
            text-cyan-50 transition-colors group-hover:bg-cyan-50 group-hover:text-cyan-500"
        >
          <FolderKanban></FolderKanban>
        </div>
        <h3 class="text-brand text-lg font-bold">
          {{ group.title }}
        </h3>
        <p class="mt-2 line-clamp-2 text-sm text-white">
          {{ group.description }}
        </p>
        <div
          class="text-brand mt-6 flex items-center justify-between border-t border-slate-50 pt-4
            text-xs font-bold"
        >
          <span>{{ $t("ui.schedulePage.itemCount", { count: group.items.length }) }}</span>
          <ArrowRight :size="16" />
        </div>
      </div>
    </div>
  </div>

  <!-- 视图：日程详情列表 -->
  <div v-if="uiStore.scheduleView === 'schedule_detail'" class="mx-auto max-w-3xl space-y-4">
    <div v-if="activeGroup.items.length === 0" class="py-20 text-center text-slate-300">
      <Inbox class="mx-auto mb-4 h-10 w-10 opacity-20" />
      <p>{{ $t("ui.schedulePage.emptyItems") }}</p>
    </div>
    <div
      v-for="(item, idx) in activeGroup.items"
      :key="idx"
      class="glass-effect flex items-start space-x-4 rounded-2xl border border-slate-100 p-5
        shadow-sm"
    >
      <div class="self-center rounded-lg bg-cyan-500 px-3 py-1 text-xs font-bold text-white">
        {{ item.time }}
      </div>
      <div class="flex-1">
        <h4 class="text-brand text-lg font-bold">{{ item.name }}</h4>
        <p class="mt-1 text-sm text-white">{{ item.content }}</p>
      </div>
      <button @click="removeScheduleItem(idx)" class="p-1 text-slate-300 hover:text-red-400">
        <Trash2 />
      </button>
    </div>
  </div>

  <!-- 引入通用模态框 -->
  <BaseModal
    :show="showModal"
    :title="modalTitle"
    @close="showModal = false"
    @confirm="confirmCreate"
  >
    <!-- 场景1：新建日程组 -->
    <template v-if="uiStore.scheduleView === 'schedule_groups'">
      <input
        v-model="formData.groupTitle"
        :placeholder="$t('ui.schedulePage.groupNamePlaceholder')"
        class="w-full rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all outline-none
          focus:ring-2 focus:ring-cyan-500/50"
      />
      <textarea
        v-model="formData.groupDesc"
        :placeholder="$t('ui.schedulePage.groupDescPlaceholder')"
        rows="3"
        class="w-full resize-none rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all
          outline-none focus:ring-2 focus:ring-cyan-500/50"
      ></textarea>
    </template>

    <!-- 场景2：新建日程项 -->
    <template v-else>
      <input
        v-model="formData.itemName"
        :placeholder="$t('ui.schedulePage.itemNamePlaceholder')"
        class="w-full rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all outline-none
          focus:ring-2 focus:ring-cyan-500/50"
      />
      <input
        v-model="formData.itemTime"
        type="time"
        class="w-full rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all outline-none
          focus:ring-2 focus:ring-cyan-500/50"
      />
      <textarea
        v-model="formData.itemContent"
        :placeholder="$t('ui.schedulePage.itemContentPlaceholder')"
        rows="2"
        class="w-full resize-none rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all
          outline-none focus:ring-2 focus:ring-cyan-500/50"
      ></textarea>
    </template>
  </BaseModal>
</template>

<script setup lang="ts">
  import { ref, computed, reactive, watch, onMounted } from "vue";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { ArrowRight, Trash2, FolderKanban, Inbox } from "lucide-vue-next";
  import { getSchedules, saveSchedules } from "@/api/services/schedule";
  import { useI18n } from "vue-i18n";

  import BaseModal from "@/components/ui/BaseModal.vue";

  const { t } = useI18n();
  const uiStore = useUIStore();

  // 数据存储
  interface ScheduleItem {
    name: string;
    time: string;
    content: string;
  }

  interface ScheduleGroup {
    title: string;
    description: string;
    items: ScheduleItem[];
  }

  const scheduleGroups = ref<Record<string, ScheduleGroup>>({});
  const loaded = ref(false);
  const preventFirstSave = ref(true);

  const loadData = async () => {
    try {
      const data = await getSchedules();
      if (data && data.scheduleGroups) {
        scheduleGroups.value = data.scheduleGroups;
      }
    } catch (e) {
      console.error("Failed to load schedules", e);
    } finally {
      loaded.value = true;
    }
  };

  watch(
    scheduleGroups,
    async (newVal) => {
      if (!loaded.value) return;
      if (preventFirstSave.value) {
        preventFirstSave.value = false;
        return;
      }
      try {
        await saveSchedules({ scheduleGroups: newVal });
      } catch (e) {
        console.error("Failed to save schedules", e);
      }
    },
    { deep: true }
  );

  onMounted(() => {
    loadData();
  });

  const activeGroup = computed(() => {
    if (!selectedGroupId.value) {
      return { items: [] };
    }
    return scheduleGroups.value[selectedGroupId.value] || { items: [] };
  });

  const removeScheduleItem = (idx: number) => {
    activeGroup.value.items.splice(idx, 1);
  };

  const removeScheduleGroup = (id: string) => {
    delete scheduleGroups.value[id];
  };

  const selectedGroupId = ref<string | null>(null);

  const selectGroup = (id: string) => {
    selectedGroupId.value = id;
    uiStore.scheduleView = "schedule_detail";
  };

  // 模态框状态
  const showModal = ref(false);
  const formData = reactive({
    groupTitle: "",
    groupDesc: "",
    itemName: "",
    itemTime: "",
    itemContent: "",
  });

  // 动态标题
  const modalTitle = computed(() => {
    return uiStore.scheduleView === "schedule_groups"
      ? t("ui.schedulePage.newGroup")
      : t("ui.schedulePage.newItem");
  });

  // 父组件调用的方法
  const handleCreate = () => {
    // 重置表单
    formData.groupTitle = "";
    formData.groupDesc = "";
    formData.itemName = "";
    formData.itemTime = "";
    formData.itemContent = "";

    showModal.value = true;
  };

  // 确认创建逻辑
  const confirmCreate = () => {
    if (uiStore.scheduleView === "schedule_groups") {
      // 创建主题逻辑
      const newId = "g" + Date.now();
      scheduleGroups.value[newId] = {
        title: formData.groupTitle,
        description: formData.groupDesc,
        items: [],
      };
    } else if (selectedGroupId.value) {
      // 创建日程项逻辑
      const group = scheduleGroups.value[selectedGroupId.value];
      if (group) {
        group.items.push({
          name: formData.itemName,
          time: formData.itemTime,
          content: formData.itemContent,
        });
      }
    }
    showModal.value = false;
  };

  defineExpose({ handleCreate });
</script>
