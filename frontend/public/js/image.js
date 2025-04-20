const qinling = document.getElementById("qinling");
const qinlingtest = document.getElementById("qinlingtest");
const filterKuosan = document.getElementById("filter-kuosan");
const haloToggle = document.getElementById("halo-toggle");

// 更新预览区域的背景
function updatePreviewBackground() {
  // 获取当前背景
  const currentBgClass = document.body.className;
  const kuosanPreview = document.getElementById("kuosan-preview");
  const kuosanTest = document.getElementById("kuosan-test");
  
  if (kuosanPreview && kuosanTest) {
    // 设置预览区域的背景样式
    let backgroundImagePath = '';
    
    // 判断是否是自定义背景
    if (currentBgClass.includes('bg-custom-')) {
      // 从localStorage获取自定义背景数据
      const customId = currentBgClass.replace('bg-custom-', '');
      const customBgData = localStorage.getItem(`customBg_${customId}`);
      
      if (customBgData) {
        const { dataUrl } = JSON.parse(customBgData);
        backgroundImagePath = dataUrl;
      } else {
        // 如果找不到自定义背景数据，则使用默认背景
        backgroundImagePath = '../pictures/backgrounds/homepage_bg.jpeg';
      }
    } 
    // 内置背景
    else if (currentBgClass.includes('bg-default')) {
      backgroundImagePath = '../pictures/backgrounds/homepage_bg.jpeg'; // 默认背景
    } else if (currentBgClass.includes('bg-night')) {
      backgroundImagePath = '../pictures/backgrounds/homepage_bg2.jpg'; // 夜间背景
    } else if (currentBgClass.includes('bg-white')) {
      backgroundImagePath = '../pictures/backgrounds/纯白背景.png';
    }
    
    console.log("image.js - 当前背景类:", currentBgClass, "路径:", backgroundImagePath);
    
    // 应用背景到预览区域
    kuosanPreview.style.backgroundImage = `url(${backgroundImagePath})`;
    kuosanPreview.style.backgroundSize = 'cover';
    kuosanPreview.style.backgroundPosition = 'center';
    kuosanPreview.style.borderRadius = '8px';
    kuosanPreview.style.padding = '15px';
    kuosanPreview.style.boxShadow = '0 4px 8px rgba(0, 0, 0, 0.2)';
    
    // 同步背景到大预览区域
    document.documentElement.style.setProperty('--current-bg', `url(${backgroundImagePath})`);
  }
}

function initImage() {
  // 获取背景页面元素
  const imagePage = document.getElementById("background-page");
  
  // 加载自定义背景
  loadCustomBackgrounds();
  
  // 切换显示/隐藏
  menuImage.addEventListener("click", () => {
    // 背景页面不使用动画
    menuImage.classList.add("show");
    imagePage.classList.add("show");
    menuSound.classList.remove("show");
    soundPage.classList.remove("show");
    historyToggle.classList.remove("show");
    historyContent.classList.remove("show");
    clearHistoryBtn.classList.remove("show");
    menuText.classList.remove("show");
    textPage.classList.remove("show");
    renderHistory();
    
    // 打开背景设置页面时更新预览背景
    updatePreviewBackground();
  });

  if (filterKuosan) {
    // 为滑块添加过渡效果
    filterKuosan.style.transition = 'all 0.2s ease';
    
    // 修改滑块最大值为5（原来的10%）
    filterKuosan.max = "5";
    
    // 初始化时更新预览背景
    updatePreviewBackground();
    
    // 鼠标悬停时只改变光标样式
    filterKuosan.addEventListener('mouseover', function() {
      this.style.cursor = 'grab';
    });
    
    // 鼠标离开时恢复
    filterKuosan.addEventListener('mouseout', function() {
      this.style.cursor = 'default';
    });
    
    // 鼠标按下时效果
    filterKuosan.addEventListener('mousedown', function() {
      this.style.cursor = 'grabbing';
    });
    
    // 鼠标释放时效果
    filterKuosan.addEventListener('mouseup', function() {
      this.style.cursor = 'grab';
    });
    
    // 直接监听input事件，实时更新设置
    filterKuosan.addEventListener("input", function () {
      // 确保值是整数并限制在合法范围内（最大值改为5）
      const rawValue = parseInt(this.value, 10);
      const Kuosan = Math.max(0, Math.min(5, rawValue || 0));
      
      // 如果解析后的值与滑块当前值不一致，更新滑块值
      if (this.value != Kuosan) {
        this.value = Kuosan;
      }
      
      // 保存选择
      localStorage.setItem("Kuosan", Kuosan);
      // 应用设置
      applyKuosan(Kuosan);
    });
    
    // 仍然保留change事件，用于记录日志
    filterKuosan.addEventListener("change", function () {
      // 记录最终值
      console.log(`光晕扩散设置为: ${this.value}`);
    });
  }

  function applyKuosan(Kuosan) {
    // 确保Kuosan是数字并限定在0-5范围内（原来是0-50）
    let kuosanValue = Number(Kuosan);
    if (isNaN(kuosanValue)) {
        kuosanValue = 2; // 默认值调整为2（原来18的10%左右）
    }
    kuosanValue = Math.max(0, Math.min(5, kuosanValue));
    
    console.log("应用光晕扩散效果:", kuosanValue);
    
    // 检查圣光效果是否启用
    const haloEnabled = localStorage.getItem("enableHalo") !== "false";
    
    // 直接使用0-5的范围
    const kuosanPreview = document.getElementById('kuosan-preview');
    const previewImage = document.getElementById('kuosan-preview-img');
    const mainImage = document.getElementById('qinling');
    
    if (kuosanPreview && previewImage) {
        // 根据开关状态应用或移除效果
        if (haloEnabled) {
            // 直接应用值，无需缩放
            previewImage.style.filter = `brightness(1.1) saturate(1.25) drop-shadow(0 0 ${kuosanValue}px rgba(255, 255, 255, 0.8))`;
            
            // 移除图像缩放效果，保持图像大小恒定
            previewImage.style.transform = "scale(1)";
        } else {
            // 移除所有效果
            previewImage.style.filter = "none";
            previewImage.style.transform = "scale(1)";
        }
        
        previewImage.style.transition = "filter 0.3s ease-in-out";
        
        // 存储值到localStorage
        localStorage.setItem('KuosanValue', kuosanValue);
    }
    
    // 同时应用到主界面图像，只修改滤镜效果，不改变大小
    if (mainImage) {
        if (haloEnabled) {
            mainImage.style.filter = `drop-shadow(0 0 ${kuosanValue}px rgba(255, 255, 255, 0.8))`;
        } else {
            mainImage.style.filter = "none";
        }
    }
  }
  
  // 初始化圣光扩散设置
  function initKuosan() {
    // 从localStorage获取保存的值
    let savedKuosan = localStorage.getItem('KuosanValue');
    // 如果有保存的值，则应用它（确保在新范围内）
    if (savedKuosan !== null) {
        const kuosanSlider = document.getElementById('filter-kuosan');
        if (kuosanSlider) {
            // 确保值在新范围内（0-5）
            savedKuosan = Math.min(5, Number(savedKuosan));
            kuosanSlider.value = savedKuosan;
            applyKuosan(savedKuosan);
        }
    } else {
        // 如果没有保存的值，使用新的默认值2
        const kuosanSlider = document.getElementById('filter-kuosan');
        if (kuosanSlider) {
            kuosanSlider.value = 2;
            applyKuosan(2);
        }
    }
  }

  // 初始化圣光开关
  if (haloToggle) {
    // 从本地存储获取圣光效果设置
    const savedHaloSetting = localStorage.getItem("enableHalo");
    haloToggle.checked = savedHaloSetting !== "false";
    
    // 应用初始设置
    applyKuosan(filterKuosan.value);
    
    // 添加过渡效果
    const haloLabel = document.querySelector('label[for="halo-toggle"]');
    if (haloLabel) {
      haloLabel.parentElement.style.transition = 'all 0.3s ease';
    }
    
    // 获取滑动条容器元素
    const kuosanContainer = filterKuosan.parentElement;
    // 获取预览区域元素
    const kuosanPreview = document.getElementById("kuosan-preview");
    
    if (kuosanContainer) {
      // 添加平滑过渡效果，包括宽度变化和透明度
      kuosanContainer.style.transition = 'opacity 0.3s ease, visibility 0.3s ease, width 0.3s ease, margin-right 0.3s ease, height 0.3s ease, margin-bottom 0.3s ease';
      kuosanContainer.style.overflow = 'hidden';
      kuosanContainer.style.display = 'flex';
      kuosanContainer.style.alignItems = 'center';
      
      // 根据初始状态设置滑动条显示/隐藏
      if (savedHaloSetting === "false") {
        kuosanContainer.style.height = '0';
        kuosanContainer.style.width = '0';
        kuosanContainer.style.marginRight = '0';
        kuosanContainer.style.marginBottom = '0';
        kuosanContainer.style.opacity = '0';
        kuosanContainer.style.visibility = 'hidden';
        
        // 如果圣光效果关闭，预览区域上移
        if (kuosanPreview) {
          kuosanPreview.style.marginTop = '0';
        }
      } else {
        kuosanContainer.style.height = 'auto';
        kuosanContainer.style.width = '100%';
        kuosanContainer.style.marginRight = 'auto';
        kuosanContainer.style.marginBottom = '25px';
        kuosanContainer.style.opacity = '1';
        kuosanContainer.style.visibility = 'visible';
        
        // 如果圣光效果打开，预览区域显示正常间距
        if (kuosanPreview) {
          kuosanPreview.style.marginTop = '50px';
        }
      }
    }
    
    haloToggle.addEventListener('change', function() {
      // 保存设置到本地存储
      localStorage.setItem('enableHalo', this.checked);
      
      // 应用设置 - 关键是这里会应用或移除圣光效果
      applyKuosan(filterKuosan.value);
      
      // 根据开关状态显示/隐藏滑动条，并平滑移动预览区域
      if (kuosanContainer) {
        if (this.checked) {
          // 显示滑动条
          kuosanContainer.style.height = 'auto';
          kuosanContainer.style.width = '100%';
          kuosanContainer.style.marginRight = 'auto';
          kuosanContainer.style.marginBottom = '25px';
          kuosanContainer.style.opacity = '1';
          kuosanContainer.style.visibility = 'visible';
          
          // 预览区域恢复正常间距
          if (kuosanPreview) {
            setTimeout(() => {
              kuosanPreview.style.marginTop = '50px';
            }, 50); // 短暂延迟，让动画顺序更合理
          }
        } else {
          // 隐藏滑动条
          kuosanContainer.style.height = '0';
          kuosanContainer.style.width = '0';
          kuosanContainer.style.marginRight = '0';
          kuosanContainer.style.marginBottom = '0';
          kuosanContainer.style.opacity = '0';
          
          // 预览区域上移
          if (kuosanPreview) {
            kuosanPreview.style.marginTop = '0';
          }
          
          setTimeout(() => {
            kuosanContainer.style.visibility = 'hidden';
          }, 300); // 等待过渡动画完成后再隐藏
        }
      }
    });
  }

  // 监听背景选择变化
  const bgOptions = document.querySelectorAll('.bg-option');
  if (bgOptions.length > 0) {
    bgOptions.forEach(option => {
      option.addEventListener('click', function() {
        // 当背景变化时，更新预览区域的背景
        setTimeout(() => {
          updatePreviewBackground();
        }, 100); // 短暂延迟确保背景类已更新
      });
    });
  }
  
  initKuosan();
  
  // 确保在页面加载时应用圣光效果到主图像
  const mainImage = document.getElementById('qinling');
  if (mainImage && localStorage.getItem('enableHalo') !== "false") {
    // 获取保存的值并确保在新范围内
    let kuosanValue = localStorage.getItem('KuosanValue') || 2;
    kuosanValue = Math.min(5, Number(kuosanValue)); // 确保不超过新的最大值5
    mainImage.style.filter = `drop-shadow(0 0 ${kuosanValue}px rgba(255, 255, 255, 0.8))`;
  } else if (mainImage) {
    mainImage.style.filter = "none";
  }
}

// 在页面加载时初始化
initImage();

// 添加自定义背景功能
const uploadBgBtn = document.getElementById('upload-bg-btn');
const uploadBgInput = document.getElementById('upload-bg-input');
const uploadStatus = document.getElementById('upload-status');
const bgOptions = document.querySelector('.bg-options');

// 检查是否已存在自定义背景
function loadCustomBackgrounds() {
  // 获取所有自定义背景
  const customBgs = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key.startsWith('customBg_')) {
      const bgData = JSON.parse(localStorage.getItem(key));
      customBgs.push({
        id: key.replace('customBg_', ''),
        name: bgData.name,
        dataUrl: bgData.dataUrl,
        timestamp: bgData.timestamp
      });
    }
  }
  
  // 按照时间戳排序，最新的在前面
  customBgs.sort((a, b) => b.timestamp - a.timestamp);
  
  // 移除现有的自定义背景选项
  document.querySelectorAll('.bg-option.custom').forEach(el => el.remove());
  
  // 添加自定义背景选项
  customBgs.forEach(bg => {
    addCustomBgOption(bg.id, bg.name, bg.dataUrl);
  });
}

// 添加自定义背景选项到界面
function addCustomBgOption(id, name, dataUrl) {
  const bgOption = document.createElement('div');
  bgOption.className = 'bg-option custom';
  bgOption.dataset.bg = `custom-${id}`;
  bgOption.dataset.customId = id;
  bgOption.innerHTML = `
    <img src="${dataUrl}" alt="${name}" />
    <span>${name}</span>
    <div class="delete-bg" title="删除此背景">×</div>
  `;
  
  // 添加删除按钮功能
  const deleteBtn = bgOption.querySelector('.delete-bg');
  deleteBtn.addEventListener('click', (e) => {
    e.stopPropagation(); // 阻止点击事件冒泡
    
    if (confirm(`确定要删除"${name}"吗？`)) {
      // 删除localStorage中的数据
      localStorage.removeItem(`customBg_${id}`);
      
      // 如果当前正在使用这个背景，则切换到默认背景
      if (document.body.classList.contains(`bg-custom-${id}`)) {
        document.body.className = '';
        document.body.classList.add('bg-default');
        
        // 设置默认背景图片
        document.body.style.backgroundImage = 'url(../pictures/backgrounds/homepage_bg.jpeg)';
        document.body.style.backgroundSize = 'cover';
        document.body.style.backgroundPosition = 'center';
        
        localStorage.setItem('background', 'default');
        
        // 更新背景选项的active状态
        document.querySelectorAll('.bg-option').forEach(bg => bg.classList.remove('active'));
        document.querySelector('.bg-option[data-bg="default"]').classList.add('active');
        
        // 更新圣光显示样本背景
        updatePreviewBackground();
      }
      
      // 从DOM中移除这个选项
      bgOption.remove();
    }
  });
  
  // 添加到背景选项区域
  bgOptions.appendChild(bgOption);
  
  // 添加点击事件
  bgOption.addEventListener('click', function() {
    // 移除所有active类
    document.querySelectorAll('.bg-option').forEach(bg => bg.classList.remove('active'));
    
    // 添加active类到当前选项
    this.classList.add('active');
    
    // 移除所有背景类
    document.body.className = '';
    
    // 添加自定义背景类
    document.body.classList.add(`bg-custom-${id}`);
    
    // 直接设置背景图片
    document.body.style.backgroundImage = `url(${dataUrl})`;
    document.body.style.backgroundSize = 'cover';
    document.body.style.backgroundPosition = 'center';
    
    // 保存选择
    localStorage.setItem('background', `custom-${id}`);
    localStorage.setItem('currentCustomBgId', id);
    
    // 更新圣光显示样本的背景
    const kuosanTest = document.getElementById('kuosan-test');
    if (kuosanTest) {
      kuosanTest.style.setProperty('--current-bg', `url(${dataUrl})`);
    }
    
    // 触发背景预览更新
    setTimeout(() => {
      updatePreviewBackground();
    }, 100);
  });
}

// 处理背景上传
if (uploadBgBtn && uploadBgInput) {
  uploadBgBtn.addEventListener('click', () => {
    uploadBgInput.click();
  });
  
  uploadBgInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file) return;
    
    // 检查文件类型
    if (!file.type.match('image.*')) {
      uploadStatus.textContent = '请选择图片文件';
      uploadStatus.style.color = 'red';
      return;
    }
    
    // 检查文件大小（限制为5MB）
    if (file.size > 5 * 1024 * 1024) {
      uploadStatus.textContent = '图片大小不能超过5MB';
      uploadStatus.style.color = 'red';
      return;
    }
    
    uploadStatus.textContent = '正在处理...';
    uploadStatus.style.color = 'blue';
    
    // 读取文件
    const reader = new FileReader();
    
    reader.onload = function(event) {
      const dataUrl = event.target.result;
      
      // 压缩图片
      const img = new Image();
      img.onload = function() {
        const canvas = document.createElement('canvas');
        const ctx = canvas.getContext('2d');
        
        // 调整尺寸，最大宽度/高度为1920px
        let width = img.width;
        let height = img.height;
        const maxSize = 1920;
        
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
        
        // 优化图片质量
        const compressedDataUrl = canvas.toDataURL('image/jpeg', 0.7);
        
        // 计算自定义背景ID和名称
        const timestamp = Date.now();
        const id = timestamp.toString();
        
        // 计算自定义背景名称（查找最大序号并加1）
        let maxIndex = 0;
        for (let i = 0; i < localStorage.length; i++) {
          const key = localStorage.key(i);
          if (key.startsWith('customBg_')) {
            const bgData = JSON.parse(localStorage.getItem(key));
            const match = bgData.name.match(/自定义背景(\d+)/);
            if (match && parseInt(match[1]) > maxIndex) {
              maxIndex = parseInt(match[1]);
            }
          }
        }
        const name = `自定义背景${maxIndex + 1}`;
        
        // 保存到localStorage
        localStorage.setItem(`customBg_${id}`, JSON.stringify({
          name,
          dataUrl: compressedDataUrl,
          timestamp
        }));
        
        // 添加到界面
        addCustomBgOption(id, name, compressedDataUrl);
        
        // 自动选择新上传的背景
        document.querySelectorAll('.bg-option').forEach(bg => bg.classList.remove('active'));
        const newBgOption = document.querySelector(`.bg-option[data-customId="${id}"]`);
        if (newBgOption) {
          newBgOption.classList.add('active');
          document.body.className = '';
          document.body.classList.add(`bg-custom-${id}`);
          
          // 直接设置背景图片
          document.body.style.backgroundImage = `url(${compressedDataUrl})`;
          document.body.style.backgroundSize = 'cover';
          document.body.style.backgroundPosition = 'center';
          
          localStorage.setItem('background', `custom-${id}`);
          localStorage.setItem('currentCustomBgId', id);
          
          // 更新圣光显示样本的背景
          const kuosanTest = document.getElementById('kuosan-test');
          if (kuosanTest) {
            kuosanTest.style.setProperty('--current-bg', `url(${compressedDataUrl})`);
          }
          
          // 触发背景预览更新
          setTimeout(() => {
            updatePreviewBackground();
          }, 100);
        }
        
        uploadStatus.textContent = '背景上传成功';
        uploadStatus.style.color = 'green';
        
        // 清空文件输入框
        uploadBgInput.value = '';
        
        // 3秒后清空状态消息
        setTimeout(() => {
          uploadStatus.textContent = '';
        }, 3000);
      };
      
      img.src = dataUrl;
    };
    
    reader.readAsDataURL(file);
  });
}