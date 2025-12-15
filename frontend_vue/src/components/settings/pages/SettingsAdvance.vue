<template>
  <MenuPage>
    <div
      class="h-[87vh] w-full bg-white/10 p-0 md:p-4 rounded-lg overflow-auto"
    >
      <div class="block md:grid grid-cols-[280px_1fr] h-full">
        <!-- 加载动画 -->
        <div
          v-if="isLoading"
          class="fixed inset-0 bg-white/80 flex justify-center items-center z-50"
        >
          <div
            class="border-5 border-gray-200 border-t-brand rounded-full w-12 h-12 animate-spin"
          ></div>
        </div>

        <!-- 导航菜单 (左侧) -->
        <nav
          ref="navContainerRef"
          @click="() => removeMoreMenu()"
          class="-left-full md:left-0 blur transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)] md:blur-none h-full p-5 flex flex-col justify-start gap-6.25 overflow-y-auto relative border-r border-brand md:moreMenu:left-0"
        >
          <!-- 滑动指示器 -->
          <div
            ref="indicatorRef"
            class="absolute left-5 w-[calc(100%-40px)] bg-brand rounded-lg z-0 transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)]"
          ></div>

          <div
            v-for="(categoryData, categoryName) in configData"
            :key="categoryName"
            class="flex flex-col gap-1 w-full"
          >
            <span
              class="text-base font-bold px-3.75 py-2.5 block rounded-lg mb-1 text-brand bg-white/10 backdrop-blur-xl backdrop-saturate-150 border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]"
              >{{ categoryName }}</span
            >
            <a
              v-for="(
                subcategoryData, subcategoryName
              ) in categoryData.subcategories"
              :key="subcategoryName"
              href="#"
              class="block px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
              :class="{
                active: isActive(categoryName, subcategoryName.toString()),
              }"
              @click.prevent="
                selectSubcategory(categoryName, subcategoryName.toString())
              "
            >
              {{ subcategoryName }}
            </a>
          </div>
        </nav>

        <!-- 设置内容区域 (右侧) -->
        <main
          class="flex justify-center h-full overflow-auto relative -top-full md:top-0 px-10 py-10 md:px-10 md:py-0"
        >
          <div v-if="selectedSubcategory" class="w-full active">
            <div class="pt-2.5 overflow-auto">
              <header class="pb-4 mb-6 border-b border-brand">
                <h2 class="m-0 text-2xl text-brand font-semibold">
                  {{ activeSelection.subcategory }}
                </h2>
                <p class="mt-2 text-base">
                  {{
                    selectedSubcategory.description ||
                    `修改 ${activeSelection.subcategory} 的相关配置`
                  }}
                </p>
              </header>

              <form @submit.prevent="saveSettings">
                <div
                  v-for="setting in selectedSubcategory.settings"
                  :key="setting.key"
                  class="mb-6"
                >
                  <!-- 根据 setting.type 渲染不同类型的输入控件 -->

                  <!-- Case: 布尔值 (Checkbox) -->
                  <template v-if="setting.type === 'bool'">
                    <label
                      class="inline-flex items-center cursor-pointer font-medium text-brand"
                    >
                      <input
                        class="mr-2.5 w-4 h-4"
                        type="checkbox"
                        :id="setting.key"
                        :checked="setting.value.toLowerCase() === 'true'"
                        @change="
                          updateSetting(
                            setting,
                            ($event.target as HTMLInputElement).checked
                          )
                        "
                      />
                      {{ setting.key }}
                    </label>
                    <p class="text-sm mt-1 mb-2 text-gray-300">
                      {{ setting.description || "" }}
                    </p>
                  </template>

                  <!-- Case: 文本域 (Textarea) -->
                  <template v-else-if="setting.type === 'textarea'">
                    <label
                      class="inline-flex items-center cursor-pointer font-medium text-brand"
                      :for="setting.key"
                      >{{ setting.key }}</label
                    >
                    <p class="text-sm mt-1 mb-2 text-gray-300">
                      {{ setting.description || "支持多行输入" }}
                    </p>
                    <textarea
                      :id="setting.key"
                      v-model="setting.value"
                      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
                      rows="8"
                    ></textarea>
                  </template>

                  <!-- Case: 默认文本 (Text Input) -->
                  <template v-else>
                    <label
                      class="inline-flex items-center cursor-pointer font-medium text-brand"
                      :for="setting.key"
                      >{{ setting.key }}</label
                    >
                    <p class="text-sm mt-1 mb-2 text-gray-300">
                      {{ setting.description || "" }}
                    </p>
                    <input
                      type="text"
                      :id="setting.key"
                      v-model="setting.value"
                      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
                    />
                  </template>
                </div>
              </form>

              <!-- 保存操作区域 -->
              <div
                class="w-18 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
              >
                <button @click="saveSettings">保存</button>
                <p :style="{ color: saveStatus.color }">
                  {{ saveStatus.message }}
                </p>
              </div>
            </div>
          </div>
          <div
            v-else-if="!isLoading && !Object.keys(configData).length"
            class="w-full active"
          >
            <div class="advanced-settings-container">
              <header>
                <h2 class="adv-title">加载失败</h2>
                <p class="adv-description">无法加载配置或配置为空。</p>
              </header>
            </div>
          </div>
        </main>
      </div>
    </div>
  </MenuPage>
</template>

<script setup lang="ts">
import {
  ref,
  onMounted,
  computed,
  reactive,
  watch,
  nextTick,
  getCurrentInstance,
} from "vue";
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";

// --- 响应式状态定义 ---
const isLoading = ref(false);
const configData = ref<Record<string, any>>({});
const activeSelection = reactive({
  category: null as string | null,
  subcategory: null as string | null,
});
const saveStatus = reactive({
  message: "",
  color: "var(--success-color)",
});
const instance = getCurrentInstance();

const emit = defineEmits([
  "remove-more-menu-from-b", // B 组件触发 remove 时通知父组件
]);

// --- Refs for DOM elements ---
const navContainerRef = ref<HTMLElement | null>(null);
const indicatorRef = ref<HTMLElement | null>(null);

// --- 计算属性 ---
const selectedSubcategory = computed(() => {
  if (activeSelection.category && activeSelection.subcategory) {
    return configData.value[activeSelection.category]?.subcategories[
      activeSelection.subcategory
    ];
  }
  return null;
});

// --- 方法定义 ---

const isActive = (category: string, subcategory: string) => {
  return (
    activeSelection.category === category &&
    activeSelection.subcategory === subcategory
  );
};

const selectSubcategory = (category: string, subcategory: string) => {
  activeSelection.category = category;
  activeSelection.subcategory = subcategory;
};

const updateSetting = (
  setting: { key: string; value: string },
  isChecked: boolean
) => {
  setting.value = isChecked ? "true" : "false";
};

const saveSettings = async () => {
  if (!selectedSubcategory.value) return;

  const formData: Record<string, string> = {};
  selectedSubcategory.value.settings.forEach(
    (setting: { key: string; value: string }) => {
      formData[setting.key] = setting.value;
    }
  );

  isLoading.value = true;
  saveStatus.message = "";

  try {
    const response = await fetch("/api/settings/config", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(formData),
    });

    const result = await response.json();
    if (!response.ok) throw new Error(result.detail || "保存失败");

    saveStatus.message = result.message;
    saveStatus.color = "var(--success-color)";

    await loadConfig(false);
  } catch (error: any) {
    saveStatus.message = `错误: ${error.message}`;
    saveStatus.color = "red";
  } finally {
    isLoading.value = false;
    setTimeout(() => {
      saveStatus.message = "";
    }, 5000);
  }
};

const loadConfig = async (selectFirst = true) => {
  isLoading.value = true;
  try {
    const response = await fetch("/api/settings/config");
    if (!response.ok) throw new Error("无法加载配置");

    configData.value = await response.json();

    if (selectFirst && Object.keys(configData.value).length > 0) {
      const firstCategory = Object.keys(configData.value)[0];
      const firstSubcategory = Object.keys(
        configData.value[firstCategory].subcategories
      )[0];
      if (firstCategory && firstSubcategory) {
        selectSubcategory(firstCategory, firstSubcategory);
      }
    }
  } catch (error: any) {
    console.error(error);
    saveStatus.message = `加载配置失败: ${error.message}`;
    saveStatus.color = "red";
  } finally {
    isLoading.value = false;
  }
};

// --- 导航指示器逻辑 ---
const updateIndicatorPosition = () => {
  if (!navContainerRef.value || !indicatorRef.value) return;

  // 找到当前激活的链接元素
  const activeLink = navContainerRef.value.querySelector(
    ".adv-nav-link.active"
  ) as HTMLElement;

  if (activeLink) {
    // 计算激活链接相对于导航容器的位置和大小
    const top = activeLink.offsetTop;
    const height = activeLink.offsetHeight;

    // 更新指示器的样式 解决自动消失，在值非空时才应用参数
    if (top) {
      indicatorRef.value.style.top = `${top}px`;
    }
    if (height) {
      indicatorRef.value.style.height = `${height}px`;
    }
  }
};

// --- 监听导航容器尺寸变化 ---
const setupNavResizeObserver = () => {
  if (!navContainerRef.value) {
    return;
  }

  const resizeObserver = new ResizeObserver((entries) => {
    updateIndicatorPosition();
  });

  // 监听导航容器
  resizeObserver.observe(navContainerRef.value);
};

// 监视 activeSelection 的变化，并在 DOM 更新后移动指示器
watch(
  activeSelection,
  async () => {
    // 等待 Vue 更新 DOM
    await nextTick();
    updateIndicatorPosition();
  },
  { deep: true }
);

// --- 生命周期钩子 ---
onMounted(async () => {
  await loadConfig();
  // 初始加载后，也需要更新一次指示器位置
  await nextTick();
  updateIndicatorPosition();
  setupNavResizeObserver();
});

// 2. 原生 add/removeMoreMenu 逻辑（操作 B 组件自身 DOM）
const addMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null;
  if (btnEl) {
    // console.log('B 组件执行 addMoreMenu');
    btnEl.classList.add("moreMenu");
  }
};

// 2. 修改 removeMoreMenu 函数
// 当这个函数被调用时，不仅执行自身逻辑，还要通知父组件
const removeMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null;
  if (btnEl) {
    btnEl.classList.remove("moreMenu");
  }

  // 关键：向父组件发送事件
  emit("remove-more-menu-from-b");
};

defineExpose({
  addMoreMenu,
});
</script>
