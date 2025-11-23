<template>
  <MenuPage>
    <div class="advanced-settings-box">
      <div class="advanced-settings-grid">
        <!-- 加载动画 -->
        <div v-if="isLoading" class="loader">
          <div class="spinner"></div>
        </div>

        <!-- 导航菜单 (左侧) -->
        <nav ref="navContainerRef" class="advanced-nav">
          <!-- 滑动指示器 -->
          <div ref="indicatorRef" class="adv-nav-indicator"></div>

          <div
            v-for="(categoryData, categoryName) in configData"
            :key="categoryName"
            class="adv-nav-category"
          >
            <span class="category-title">{{ categoryName }}</span>
            <a
              v-for="(
                subcategoryData, subcategoryName
              ) in categoryData.subcategories"
              :key="subcategoryName"
              href="#"
              class="adv-nav-link"
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
        <main class="advanced-content">
          <div v-if="selectedSubcategory" class="adv-content-page active">
            <div class="advanced-settings-container">
              <header>
                <h2 class="adv-title">{{ activeSelection.subcategory }}</h2>
                <p class="adv-description">
                  {{
                    selectedSubcategory.description ||
                    `修改 ${activeSelection.subcategory} 的相关配置`
                  }}
                </p>
              </header>

              <form class="settings-form" @submit.prevent="saveSettings">
                <div
                  v-for="setting in selectedSubcategory.settings"
                  :key="setting.key"
                  class="form-group"
                >
                  <!-- 根据 setting.type 渲染不同类型的输入控件 -->

                  <!-- Case: 布尔值 (Checkbox) -->
                  <template v-if="setting.type === 'bool'">
                    <label class="checkbox-label">
                      <input
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
                    <p class="description">{{ setting.description || "" }}</p>
                  </template>

                  <!-- Case: 文本域 (Textarea) -->
                  <template v-else-if="setting.type === 'textarea'">
                    <label :for="setting.key">{{ setting.key }}</label>
                    <p class="description">
                      {{ setting.description || "支持多行输入" }}
                    </p>
                    <textarea
                      :id="setting.key"
                      v-model="setting.value"
                      class="form-control"
                      rows="8"
                    ></textarea>
                  </template>

                  <!-- Case: 默认文本 (Text Input) -->
                  <template v-else>
                    <label :for="setting.key">{{ setting.key }}</label>
                    <p class="description">{{ setting.description || "" }}</p>
                    <input
                      type="text"
                      :id="setting.key"
                      v-model="setting.value"
                      class="form-control"
                    />
                  </template>
                </div>
              </form>

              <!-- 保存操作区域 -->
              <div class="save-actions">
                <button @click="saveSettings">保存</button>
                <p :style="{ color: saveStatus.color }">
                  {{ saveStatus.message }}
                </p>
              </div>
            </div>
          </div>
          <div
            v-else-if="!isLoading && !Object.keys(configData).length"
            class="adv-content-page active"
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
import { ref, onMounted, computed, reactive, watch, nextTick } from "vue";
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

    // 更新指示器的样式
    indicatorRef.value.style.top = `${top}px`;
    indicatorRef.value.style.height = `${height}px`;
  }
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
});
</script>

<style scoped>
/* --- 变量定义 (如果需要) --- */

.advanced-settings-box {
  background: rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 15px;
  width: 100%;
  max-width: var(--menu-max-width);
  height: 580px; /* 如果内容过多，可以设置最大高度和滚动条 */
}

/* --- 高级设置页面基础布局 --- */
.advanced-settings-grid {
  display: grid;
  grid-template-columns: 280px 1fr; /* 侧边栏固定宽度，内容区自适应 */
}

/* --- 高级设置侧边导航栏 --- */
.advanced-nav {
  padding: 20px;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  gap: 25px;
  overflow-y: auto; /* 当导航项过多时，使其可以独立滚动 */
  position: relative; /* 为指示器提供定位上下文 */
  border-right: 1px solid var(--accent-color);
  max-height: 550px;
  scrollbar-width: thin;
}

/* 二级导航滑动指示器 */
.adv-nav-indicator {
  position: absolute;
  top: 0; /* JS会更新 */
  left: 20px; /* 左右留出一些边距 */
  width: calc(100% - 40px); /* 左右留出一些边距 */
  height: 0; /* JS会更新 */
  background-color: var(--accent-color);
  border-radius: 6px;
  z-index: 0; /* 确保在链接文字下方 */
  transition: top 0.3s cubic-bezier(0.18, 0.89, 0.32, 1.28), height 0.3s cubic-bezier(0.18, 0.89, 0.32, 1.28); /* 替换为更加有弹性的效果 */
}

.advanced-nav .adv-nav-category {
  display: flex;
  flex-direction: column;
  gap: 5px;
  width: 100%;
}

.advanced-nav .category-title {
  font-size: 16px;
  font-weight: bold;
  padding: 10px 15px;
  display: block;
  border-radius: 8px;
  margin-bottom: 5px;

  color: var(--accent-color);
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

.advanced-nav .adv-nav-link {
  display: block;
  padding: 12px 20px;
  text-decoration: none;
  border-radius: 6px;
  color: #ffffff;
  transition: background-color 0.2s, color 0.2s;
  position: relative; /* 确保文字在指示器上方 */
  z-index: 1; /* 确保文字在指示器上方 */
}

.advanced-nav .adv-nav-link:hover:not(.active) {
  background-color: #e5e7eb;
}

.advanced-nav .adv-nav-link.active {
  color: white;
  font-weight: bold;
}

/* --- 高级设置内容区 --- */
.advanced-content {
  padding: 0 40px;
  overflow-y: auto;
  display: flex;
  justify-content: center;
  max-height: 550px;
}

.adv-content-page {
  width: 100%;
  max-width: 900px;
}

.advanced-settings-container {
  padding-top: 10px;
}

.advanced-settings-container header {
  padding-bottom: 15px;
  margin-bottom: 25px;
  border-bottom: 1px solid var(--accent-color);
}

.advanced-settings-container .adv-title {
  margin: 0;
  font-size: 24px;
  color: var(--accent-color);
  font-weight: 600;
}

.advanced-settings-container .adv-description {
  margin: 8px 0 0;
  font-size: 16px;
}

.settings-form {
  max-width: 800px; /* 限制最大宽度，在宽屏上更美观 */
}

.form-group {
  margin-bottom: 24px;
}

.form-group label:not(.checkbox-label) {
  display: block;
  font-weight: bold;
  margin-bottom: 6px;
  color: var(--accent-color);
}

.form-group .description {
  font-size: 13px;
  margin-top: 4px;
  margin-bottom: 8px;
}

.form-group .form-control {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #fff;
  border-radius: 8px;
  font-size: 15px;
  font-family: inherit;
  transition: border-color 0.2s, box-shadow 0.2s;
  resize: vertical;

  color: #fff;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

.form-group .form-control:focus {
  outline: none;
  border-color: var(--accent-color);
  box-shadow: 0 0 0 3px rgba(0, 123, 255, 0.2);
}

/* Checkbox Style */
.form-group .checkbox-label {
  display: flex;
  align-items: center;
  cursor: pointer;
  font-weight: 500;
  color: var(--accent-color);
}

.form-group .checkbox-label input[type="checkbox"] {
  margin-right: 10px;
  width: 16px;
  height: 16px;
  accent-color: var(--accent-color);
}

/* --- 保存操作区域 --- */
.save-actions {
  margin-top: 30px;
  display: flex;
  align-items: center;
  gap: 20px;
}

.save-actions button {
  padding: 10px 20px;
  background-color: var(--accent-color);
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 15px;
  font-weight: 500;
  transition: background-color 0.2s;
}

.save-actions button:hover {
  background-color: #0056b3;
}

.save-actions p {
  font-weight: bold;
}

/* --- 加载动画 --- */
.loader {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(255, 255, 255, 0.8);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
}

.spinner {
  border: 5px solid #f3f3f3;
  border-top: 5px solid var(--accent-color);
  border-radius: 50%;
  width: 50px;
  height: 50px;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}
</style>
