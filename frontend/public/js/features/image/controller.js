import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import eventListener from '../../core/event-bus.js'
import request from "../../core/request.js";



export class ImageController {
  constructor() {
    this.kuosanValue = "18"; // 默认光晕扩散值
    this.domUtils = DomUtils;
    this.maxIndex = 0;
    this.init();
  }

  init() {
    this.bindEvents();
    this.loadCustomBackgrounds();
    this.initKuosan();
    this.updatePreviewBackground();
  }

  bindEvents() {
    // 图片菜单点击事件
    DOM.menuImage?.addEventListener("click", () => this.showImagePanel());

    // 光晕扩散设置变化事件
    DOM.image.filterKuosan?.addEventListener("change", (e) => {
      this.kuosanValue = e.target.value;
      this.saveKuosanSetting();
      this.applyKuosan();
    });

    // 背景选项点击事件
    DOM.image.bgOption.forEach((option) => {
      option?.addEventListener("click", () => {
        // 短暂延迟确保背景类已更新
        setTimeout(() => this.updatePreviewBackground(), 100);
      });
    });

    // 背景上传处理
    DOM.image.uploadBgBtn?.addEventListener("click", () =>
      DOM.image.uploadBgInput?.click()
    );

    // 自定义背景上传处理
    DOM.image.uploadBgInput?.addEventListener("change", (e) => {
      const file = e.target.files[0];
      if (!file) return;

      this.handleFileUpload(file);
    });
  }

  showImagePanel() {
    this.domUtils.showElements([DOM.menuImage, DOM.imagePage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuImage, DOM.imagePage])
    );

    this.updatePreviewBackground();
  }

  applyKuosan() {
    const filterValue = `drop-shadow(0 0 ${this.kuosanValue}px rgba(255, 255, 255, 1))`;

    if (DOM.image.qinling) DOM.image.qinling.style.filter = filterValue;
    if (DOM.image.qinlingtest) DOM.image.qinlingtest.style.filter = filterValue;

    if (DOM.image.kuosanPreview && DOM.image.kousanPreviewImg) {
      DOM.image.kousanPreviewImg.style.filter = `brightness(1.1) saturate(1.25) drop-shadow(0 0 ${this.kuosanValue}px rgba(255, 255, 255, 0.8))`;
      DOM.image.kousanPreviewImg.style.transform = "scale(1)";
      DOM.image.kousanPreviewImg.style.transition = "filter 0.3s ease-in-out";

      localStorage.setItem("KuosanValue", this.kuosanValue);
    }
  }

  saveKuosanSetting() {
    localStorage.setItem("Kuosan", this.kuosanValue);
  }

  initKuosan() {
    const savedKuosan = localStorage.getItem("Kuosan") || this.kuosanValue;
    if (DOM.image.filterKuosan) DOM.image.filterKuosan.value = savedKuosan;
    this.kuosanValue = savedKuosan;

    this.applyKuosan();
  }

  updatePreviewBackground() {
    // 获取当前背景
    const currentBgClass = document.body.className;

    // 设置预览区域的背景样式
    let backgroundImagePath = "";

    // 判断是否是自定义背景
    if (currentBgClass.includes("bg-custom-")) {
      // 从localStorage获取自定义背景数据
      const customId = currentBgClass.replace("bg-custom-", "");
      const customBgData = localStorage.getItem(`customBg_${customId}`);

      if (customBgData) {
        const { dataUrl } = JSON.parse(customBgData);
        backgroundImagePath = dataUrl;
      } else {
        // 如果找不到自定义背景数据，则使用默认背景
        backgroundImagePath = "../pictures/backgrounds/homepage_bg.jpeg";
      }
    }
    // 内置背景
    else {
      document.body.style = "";
      if (currentBgClass.includes("bg-default")) {
        backgroundImagePath = "../pictures/backgrounds/homepage_bg.jpeg"; // 默认背景
      } else if (currentBgClass.includes("bg-night")) {
        backgroundImagePath = "../pictures/backgrounds/homepage_bg2.jpg"; // 夜间背景
      } else if (currentBgClass.includes("bg-white")) {
        backgroundImagePath = "../pictures/backgrounds/纯白背景.png";
      }
    }

    // 应用背景到预览区域
    DOM.image.kuosanPreview.style.backgroundImage = `url(${backgroundImagePath})`;
    DOM.image.kuosanPreview.style.backgroundSize = "cover";
    DOM.image.kuosanPreview.style.backgroundPosition = "center";
    DOM.image.kuosanPreview.style.borderRadius = "8px";
    DOM.image.kuosanPreview.style.boxShadow = "0 4px 8px rgba(0, 0, 0, 0.2)";

    // 同步背景到大预览区域
    document.documentElement.style.setProperty(
      "--current-bg",
      `url(${backgroundImagePath})`
    );
  }

  // 检查是否已存在自定义背景
  loadCustomBackgrounds() {
    return request.backgroundList()
    .then(list => {
      const customBgs = list;

      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key.startsWith("customBg_")) {
          const bgData = JSON.parse(localStorage.getItem(key));
          customBgs.push({
            id: key.replace("customBg_", ""),
            name: bgData.name,
            dataUrl: bgData.dataUrl,
            timestamp: bgData.timestamp,
          });
        }
      }
  
      // 按照时间戳排序，最新的在前面
      customBgs.sort((a, b) => b.timestamp - a.timestamp);
  
      // 移除现有的自定义背景选项
      document.querySelectorAll(".bg-option.custom").forEach((el) => el.remove());
  
      // 添加自定义背景选项
      customBgs.forEach((bg) => {
        this.addCustomBgOption(bg.id, bg.name, bg.dataUrl);
      });
    })
    
 
  }

  // 添加自定义背景选项到界面
  addCustomBgOption(id, name, dataUrl) {
    const bgOption = document.createElement("div");
    bgOption.className = "bg-option custom";
    bgOption.dataset.bg = `custom-${id}`;
    bgOption.dataset.customId = id;
    bgOption.innerHTML = `
      <img src="${dataUrl}" alt="${name}" />
      <span>${name}</span>
      <div class="delete-bg" title="删除此背景">×</div>
    `;

    // 添加删除按钮功能
    const deleteBtn = bgOption.querySelector(".delete-bg");
    deleteBtn.addEventListener("click", (e) => {
      e.stopPropagation(); // 阻止点击事件冒泡

      if (confirm(`确定要删除"${name}"吗？`)) {
        // 删除localStorage中的数据

        return request.backgroundDelete(id)
        .then(() => {

          // 如果当前正在使用这个背景，则切换到默认背景
          if (document.body.classList.contains(`bg-custom-${id}`)) {
            document.body.className = "";
            document.body.classList.add("bg-default");

            // 设置默认背景图片
            document.body.style.backgroundImage =
              "url(../pictures/backgrounds/homepage_bg.jpeg)";
            document.body.style.backgroundSize = "cover";
            document.body.style.backgroundPosition = "center";

            localStorage.setItem("background", "default");

            // 更新背景选项的active状态
            document
              .querySelectorAll(".bg-option")
              .forEach((bg) => bg.classList.remove("active"));
            document
              .querySelector('.bg-option[data-bg="default"]')
              .classList.add("active");

            // 更新圣光显示样本背景
            this.updatePreviewBackground();
            
            // 从DOM中移除这个选项
            bgOption.remove();

          }
        })
      }
    });

    // 添加到背景选项区域
    DOM.image.bgOptions.appendChild(bgOption);

    // 添加点击事件
    bgOption.addEventListener("click", () => {
      // 移除所有active类
      DOM.image.bgOption.forEach((bg) => bg.classList.remove("active"));

      // 添加active类到当前选项
      bgOption.classList.add("active");

      // 移除所有背景类
      document.body.className = "";

      // 添加自定义背景类
      document.body.classList.add(`bg-custom-${id}`);

      // 直接设置背景图片
      eventListener.emit('background:change', dataUrl)
      document.body.style.backgroundImage = `url(${dataUrl})`;
      document.body.style.backgroundSize = "cover";
      document.body.style.backgroundPosition = "center";

      // 保存选择
      localStorage.setItem("background", `custom-${id}`);
      localStorage.setItem("currentCustomBgId", id);

      // 更新圣光显示样本的背景
      DOM.image.kuosanTest.style.setProperty("--current-bg", `url(${dataUrl})`);

      // 触发背景预览更新
      setTimeout(() => this.updatePreviewBackground(), 100);
    });
  }

  handleFileUpload(file) {
    // 检查文件类型
    if (!file.type.match("image.*")) {
      DOM.image.uploadStatus.textContent = "请选择图片文件";
      DOM.image.uploadStatus.style.color = "red";
      return;
    }

    // 检查文件大小（限制为5MB）
    if (file.size > 5 * 1024 * 1024) {
      DOM.image.uploadStatus.textContent = "图片大小不能超过5MB";
      DOM.image.uploadStatus.style.color = "red";
      return;
    }

    DOM.image.uploadStatus.textContent = "正在处理...";
    DOM.image.uploadStatus.style.color = "blue";


    const formData = new FormData();
    formData.append("file", file);
    return request.backgroundUpload(formData)
    .then(() => {
      alert("图片上传成功");
    })
    .catch(error => {
      console.error("上传图片错误:", error);
      alert("图片上传失败");
    })
    // const reader = new FileReader();
    // reader.onload = (event) => this.processImage(event.target.result);
    // reader.readAsDataURL(file);
  }

  processImage(dataUrl) {
    const img = new Image();
    img.onload = () => {
      const compressedDataUrl = this.compressImage(img);
      const { id, name } = this.generateBgInfo();

      this.saveCustomBackground(id, name, compressedDataUrl);
      this.applyNewBackground(id, compressedDataUrl);

      DOM.image.uploadStatus.textContent = "背景上传成功";
      DOM.image.uploadStatus.style.color = "green";
      DOM.image.uploadBgInput.value = "";

      setTimeout(() => {
        DOM.image.uploadStatus.textContent = "";
      }, 3000);
    };
    img.src = dataUrl;
  }

  compressImage(img) {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    const maxSize = 1920;
    let { width, height } = img;

    if (width > maxSize || height > maxSize) {
      if (width > height) {
        height = Math.round(height * (maxSize / width));
        width = maxSize;
      } else {
        width = Math.round(width * (maxSize / height));
        height = maxSize;
      }
    }

    canvas.width = width;
    canvas.height = height;
    ctx.drawImage(img, 0, 0, width, height);

    return canvas.toDataURL("image/jpeg", 0.7);
  }

  generateBgInfo() {
    const timestamp = Date.now();
    const id = timestamp.toString();

    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key.startsWith("customBg_")) {
        const bgData = JSON.parse(localStorage.getItem(key));
        const match = bgData.name.match(/自定义背景(\d+)/);
        if (match && parseInt(match[1]) > this.maxIndex) {
          this.maxIndex = parseInt(match[1]);
        }
      }
    }

    return { id, name: `自定义背景${this.maxIndex + 1}` };
  }

  saveCustomBackground(id, name, dataUrl) {
    localStorage.setItem(
      `customBg_${id}`,
      JSON.stringify({
        name,
        dataUrl,
        timestamp: Date.now(),
      })
    );
  }

  applyNewBackground(id, dataUrl) {
    DOM.image.bgOption?.forEach((bg) => bg.classList.remove("active"));

    this.addCustomBgOption(id, `自定义背景${this.maxIndex + 1}`, dataUrl);

    const newBgOption = document.querySelector(
      `.bg-option[data-customId="${id}"]`
    );
    if (!newBgOption) return;

    newBgOption.classList.add("active");
    document.body.className = "";
    document.body.classList.add(`bg-custom-${id}`);
    document.body.style.backgroundImage = `url(${dataUrl})`;
    document.body.style.backgroundSize = "cover";
    document.body.style.backgroundPosition = "center";

    localStorage.setItem("background", `custom-${id}`);
    localStorage.setItem("currentCustomBgId", id);

    DOM.image.kuosanTest?.style.setProperty("--current-bg", `url(${dataUrl})`);
    setTimeout(() => this.updatePreviewBackground(), 100);
  }
}
