<template>
  <div class="settings-page-ex">
    <div class="acrylic-background"></div>

    <div class="advanced-settings-box">
      <div class="nav-header">LingChat-Pal 设置</div>
      <div class="advanced-settings-grid">
        <!-- 导航菜单 (左侧) -->
        <nav ref="navContainerRef" class="advanced-nav">
          <!-- 滑动指示器 -->
          <div ref="indicatorRef" class="adv-nav-indicator"></div>

          <div class="adv-nav-category">
            <span class="category-title">日程</span>
            <a
              href="#"
              class="adv-nav-link active"
              @click.prevent="
                selectSubcategory('日程'.toString(), '日程设置'.toString())
              "
              >日程设置</a
            >
          </div>
        </nav>

        <!-- 设置内容区域 (右侧) -->
        <main class="advanced-content">
          <div class="adv-content-page active">
            <div class="advanced-settings-container">
              <header>
                <h2 class="adv-title">日程设置</h2>
                <p class="adv-description">
                  在这里设定灵灵和你的日常日程安排，让灵灵更好地协助你管理时间和任务。
                </p>
              </header>

              <form class="settings-form">
                <div class="form-group">
                  <label>日程标题</label>
                  <p class="description">为你的日程设定一个标题吧~</p>
                  <input type="text" id="schedule-title" class="form-control" />
                </div>
              </form>
            </div>
          </div>
        </main>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive, watch, nextTick } from "vue";

const activeSelection = reactive({
  category: null as string | null,
  subcategory: null as string | null,
});

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
</script>

<style scoped>
/* --- 变量定义 (如果需要) --- */
.settings-page-ex {
  width: 95%;
  height: 95%;
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 20px;
  color: white;

  background: linear-gradient(135deg, #000c14 0%, #131225 50%, #170919 100%);
  backdrop-filter: blur(20px) saturate(150%);
  -webkit-backdrop-filter: blur(20px) saturate(150%);

  border-radius: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  border: 3px solid rgba(255, 255, 255, 0.125);

  text-shadow: #000c14 1px 1px 2px;
}

.acrylic-background {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(20px) saturate(150%);
  -webkit-backdrop-filter: blur(20px) saturate(150%);
  z-index: -1;

  border-radius: 12px;
}

.nav-header {
  font-size: 20px;
  font-weight: bold;
  padding: 10px 20px;
  color: white;
  border-bottom: 5px solid var(--accent-color);
  width: 95%;

  -webkit-app-region: drag;
}

.advanced-settings-box {
  padding: 15px;
  width: 100%;
  height: 100%; /* 如果内容过多，可以设置最大高度和滚动条 */
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
  -ms-overflow-style: -ms-autohiding-scrollbar;
}

/* --- 高级设置页面基础布局 --- */
.advanced-settings-grid {
  display: grid;
  grid-template-columns: 280px 1fr; /* 侧边栏固定宽度，内容区自适应 */
  height: calc(100% - 70px); /* 减去标题和内边距的高度 */
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
  height: 100%;
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
  transition: top 0.3s ease-in-out, height 0.3s ease-in-out;
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
  padding: 20px 40px;
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
