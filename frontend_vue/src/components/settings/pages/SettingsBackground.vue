<template>
  <MenuPage>
    <MenuItem title="背景选择">
      <div class="background-container">
        <div class="background-list character-grid">
          <div
            v-for="(background, index) in backgroundList"
            :key="index"
            :class="[
              'background-card',
              { selected: isSelected(background.url) },
            ]"
          >
            <div class="background-image-container">
              <img
                :src="background.url"
                :alt="background.title"
                class="background-image"
              />
            </div>
            <div class="background-title" :data-title="background.title">
              <Button
                :class="[
                  'background-select-btn',
                  { selected: isSelected(background.url) },
                ]"
                @click="selectBackground(background.url)"
              >
                {{ isSelected(background.url) ? "已选中" : "选择" }}
              </Button>
            </div>
          </div>
        </div>

        <Button type="big" @click="triggerUpload">上传自定义背景</Button>
        <input
          type="file"
          ref="uploadInput"
          @change="handleFileUpload"
          accept=".jpg,.png,.webp,.bmp,.svg,.tif,.gif"
          style="display: none"
        />
      </div>
    </MenuItem>

    <MenuItem title="粒子选择" size="large">
      <div class="effect-list">
        <Button type="big" @click="updateParticle(`StarField`)">星空</Button>
        <Button type="big" @click="updateParticle(`Rain`)">雨水</Button>
        <Button type="big" @click="updateParticle(`Sakura`)">樱花</Button>
        <Button type="big" @click="updateParticle(`Snow`)">雪景</Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, nextTick } from "vue";
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";
import { Button } from "../../base";
import { Slider } from "../../base";
import { getBackgroundImages } from "../../../api/services/background";
import { BackgroundImageInfo } from "../../../types";
import { useUIStore } from "../../../stores/modules/ui/ui";

// 响应式数据
const backgroundList = ref<BackgroundImageInfo[]>([]);
const selectedBackground = ref<string>("");
const uploadInput = ref<HTMLInputElement | null>(null);

const uiStore = useUIStore();

// 初始化
onMounted(async () => {
  try {
    await refreshBackground();

    // 检查本地存储中是否有已选背景
    const savedBg = localStorage.getItem("selectedBackground");
    if (savedBg) {
      selectBackground(savedBg);
    } else if (backgroundList.value.length > 0) {
      // 随机选择一个背景
      const randomIndex = Math.floor(
        Math.random() * backgroundList.value.length
      );
      selectBackground(backgroundList.value[randomIndex].url);
      console.log("已选随机背景");
    }
  } catch (error) {
    console.error("加载背景图片失败", error);
  }
});

// 获取背景列表
async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
  try {
    const data = await getBackgroundImages();
    return data.map((background: BackgroundImageInfo) => ({
      title: background.title ? background.title : "草泥马",
      url: background.url
        ? `/api/v1/chat/background/background_file/${encodeURIComponent(
            background.url
          )}`
        : "../pictures/background/default.png",
      time: background.time,
    }));
  } catch (error) {
    console.error("获取背景列表失败:", error);
    return [];
  }
}

// 刷新背景列表
async function refreshBackground(): Promise<void> {
  backgroundList.value = await fetchBackgrounds();
}

// 检查是否已选中
function isSelected(url: string): boolean {
  return selectedBackground.value === url;
}

// 选择背景
function selectBackground(url: string): void {
  selectedBackground.value = url;
  // 更新DOM背景
  uiStore.currentBackground = url;
  // 保存到本地存储
  localStorage.setItem("selectedBackground", url);
}

// 触发文件上传
function triggerUpload(): void {
  if (uploadInput.value) {
    uploadInput.value.click();
  }
}

// 处理文件上传
async function handleFileUpload(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  const fileName = file.name;
  const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();

  const allowedExts = [".jpg", ".png", ".webp", ".bmp", ".svg", ".tif", ".gif"];

  if (!allowedExts.includes(fileExt)) {
    alert("请上传支持的图片格式: " + allowedExts.join(", "));
    return;
  }

  const formData = new FormData();
  formData.append("file", file);
  formData.append("name", fileName);

  try {
    const response = await fetch("/api/v1/chat/background/upload", {
      method: "POST",
      body: formData,
    });

    if (!response.ok) throw new Error("上传失败");

    await refreshBackground();

    // 清空input值，允许重复上传同一文件
    if (target) target.value = "";
  } catch (error) {
    console.error("上传失败", error);
    alert("上传失败，请重试");
  }
}

// 更新圣光效果
function updateKousan(value: number): void {}

function updateParticle(value: string): void {
  uiStore.currentBackgroundEffect = value;
}
</script>

<style scoped>
/* 确保网格容器正确 */
.backgrounds-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

/*什么？你问我为什么这里是character-grid? 灵式编程懂不懂！ */
.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  width: 100%;
}

/* 卡片容器 */
/* 为整个卡片添加渐变背景，增强毛玻璃效果 */
.background-card {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  transition: all 0.3s ease;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

/* 图片容器 */
/* 图片容器添加伪元素增强效果 */
.background-image-container::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(
    to bottom,
    transparent 60%,
    rgba(0, 0, 0, 0.3) 100%
  );
  z-index: 1;
  pointer-events: none;
}

.background-image-container {
  flex: 1; /* 占据剩余空间 */
  position: relative;
  overflow: hidden;
}

/* 图片样式 */
.background-image {
  position: relative;
  width: 100%;
  height: 100%;
  object-fit: cover;
  aspect-ratio: 16/9; /* 保持图片比例 */
  transition: transform 0.3s ease;
}

/* 底部信息区域 */
/* 修改卡片底部背景为毛玻璃效果 */
.background-title {
  padding: 12px 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(255, 255, 255, 0.15); /* 更透明的背景 */
  backdrop-filter: blur(10px); /* 毛玻璃效果 */
  -webkit-backdrop-filter: blur(10px); /* Safari 支持 */
  border-top: 1px solid rgba(255, 255, 255, 0.2); /* 更柔和的边框 */
  position: relative;
  z-index: 2;
}
/* 标题文本样式 */
/* 标题文字颜色调整以适应毛玻璃背景 */
.background-title::before {
  content: attr(data-title);
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9); /* 更亮的文字颜色 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 70%;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

/* 选择按钮 */
.background-select-btn {
  padding: 6px 12px;
  background: #4f46e5;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  flex-shrink: 0;
}

/* 交互效果 */
.background-card:hover {
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(25px) saturate(200%);
  transform: translateY(-4px) scale(1.01);
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.15),
    inset 0 2px 2px rgba(255, 255, 255, 0.15);
}

.background-card:hover .background-image {
  transform: scale(1.03);
}

.background-select-btn:hover {
  background: #4338ca;
  transform: translateY(-1px);
}

.background-select-btn:active {
  transform: translateY(0);
}

.effect-list {
  display: flex;
  justify-content: space-around;
  gap: 20px;
  align-items: center;
}

/* 响应式调整 */
@media (max-width: 768px) {
  #backgrounds-list {
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 16px;
    padding: 12px;
  }
}

@media (max-width: 480px) {
  #backgrounds-list {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  /*什么？你问我为什么这里是character-grid? 灵式编程懂不懂！ */
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }
}

/* 选中状态的卡片样式 */
.background-card.selected {
  border: 2px solid #3bc7f6d8;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.3);
}

/* 已选中按钮样式 */
.background-select-btn.selected {
  background-color: #10b981 !important;
}
</style>
