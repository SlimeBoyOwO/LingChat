// 菜单功能实现
document.addEventListener("DOMContentLoaded", function () {
  const menuToggle = document.getElementById("menu-toggle");
  const menuContent = document.getElementById("menu-content");
  const menuText = document.getElementById("menu-text");
  const textPage = document.getElementById("text-page");
  const closeMenu = document.getElementById("close-menu");
  const testMessage = document.getElementById("testmessage");
  
  // 创建页面切换动画元素
  const pageTransition = document.createElement("div");
  pageTransition.className = "page-transition";
  // 使用WebP序列帧创建动画容器
  pageTransition.innerHTML = `<div class="webp-animation-container"></div>`;
  document.body.appendChild(pageTransition);
  
  // 获取动画容器元素
  const webpContainer = pageTransition.querySelector(".webp-animation-container");
  
  // 动画开关设置
  let enableAnimation = localStorage.getItem("enableAnimation") !== "false";
  
  // 获取所有菜单按钮 - 这些变量已在顶部定义或在history.js中定义
  // 确保引用正确的变量
  const menuImage = document.getElementById("menu-image");
  const imagePage = document.getElementById("menu-image-page") || document.getElementById("background-page");
  const menuSound = document.getElementById("menu-sound");
  const soundPage = document.getElementById("sound-page");
  const historyToggle = document.getElementById("menu-history");
  const historyContent = document.getElementById("history-page");
  const clearHistoryBtn = document.getElementById("clear-history");
  
  // 页面切换动画函数
  function playPageTransition(callback) {
    // 如果动画已禁用，直接执行回调
    if (!enableAnimation) {
      if (callback && typeof callback === "function") {
        callback();
      }
      return;
    }
    
    pageTransition.classList.add("active");
    
    // 播放WebP帧序列动画 - 使用33ms的帧间隔实现1.5倍速度
    playWebpSequence(0, 39, 33, () => {
      pageTransition.classList.remove("active");
      if (callback && typeof callback === "function") {
        callback();
      }
    });
  }
  
  // 播放WebP帧序列动画
  function playWebpSequence(startFrame, endFrame, frameDelay, callback, speedMultiplier = 1.5) {
    if (!webpContainer) return;
    
    // 清空容器
    webpContainer.innerHTML = '';
    
    let currentFrame = startFrame;
    const img = document.createElement('img');
    img.className = 'webp-frame';
    img.style.width = '100%';
    img.style.height = '100%';
    img.style.objectFit = 'cover';
    
    webpContainer.appendChild(img);
    
    // 预加载帧
    const preloadImages = [];
    for (let i = startFrame; i <= endFrame; i++) {
      const preloadImg = new Image();
      preloadImg.src = `../pictures/donghua/${i}.webp`;
      preloadImages.push(preloadImg);
    }
    
    // 计算帧递增步长，用于跳帧实现更精确的倍速播放
    const frameStep = speedMultiplier;
    let frameCounter = 0;
    
    // 播放帧序列
    const playFrame = () => {
      if (currentFrame <= endFrame) {
        // 计算当前应该显示的帧索引
        const frameIndex = Math.min(Math.floor(currentFrame), endFrame);
        img.src = `../pictures/donghua/${frameIndex}.webp`;
        
        // 根据倍速增加帧计数
        frameCounter += frameStep;
        currentFrame = startFrame + frameCounter;
        
        setTimeout(playFrame, frameDelay);
      } else {
        // 动画结束
        if (callback && typeof callback === 'function') {
          callback();
        }
      }
    };
    
    // 开始播放
    playFrame();
  }

  // 切换显示/隐藏
  menuToggle.addEventListener("click", () => {
    playPageTransition(() => {
      menuContent.classList.add("show");
      
      // 确保只有文本页面显示，其他页面隐藏
      menuText.classList.add("show");
      textPage.classList.add("show");
      
      historyToggle.classList.remove("show");
      historyContent.classList.remove("show");
      clearHistoryBtn.classList.remove("show");
      
      menuImage.classList.remove("show");
      imagePage.classList.remove("show");
      
      menuSound.classList.remove("show");
      soundPage.classList.remove("show");
      
      if (menuContent.classList.contains("show")) {
        renderHistory();
      }
    });
  });

  // 切换到文本页面
  menuText.addEventListener("click", () => {
    // 文本页面不使用动画
    menuText.classList.add("show");
    textPage.classList.add("show");
    
    // 隐藏其他所有页面
    historyToggle.classList.remove("show");
    historyContent.classList.remove("show");
    clearHistoryBtn.classList.remove("show");
    
    menuImage.classList.remove("show");
    imagePage.classList.remove("show");
    
    menuSound.classList.remove("show");
    soundPage.classList.remove("show");
    
    if (menuContent.classList.contains("show")) {
      renderHistory();
    }
  });

  // 切换显示/隐藏
  closeMenu.addEventListener("click", () => {
    // 关闭菜单不使用动画
      menuContent.classList.remove("show");
      menuText.classList.remove("show");
      textPage.classList.remove("show");
      historyToggle.classList.remove("show");
      historyContent.classList.remove("show");
      clearHistoryBtn.classList.remove("show");
      menuImage.classList.remove("show");
      imagePage.classList.remove("show");
      menuSound.classList.remove("show");
      soundPage.classList.remove("show");
    });

  // 添加历史页面切换逻辑
  historyToggle.addEventListener("click", () => {
    // 先隐藏所有其他页面
    menuText.classList.remove("show");
    textPage.classList.remove("show");
    
    menuImage.classList.remove("show");
    imagePage.classList.remove("show");
    
    menuSound.classList.remove("show");
    soundPage.classList.remove("show");
    
    // 然后显示历史页面
    historyToggle.classList.add("show");
    historyContent.classList.add("show");
    clearHistoryBtn.classList.add("show");
    
    // 渲染历史记录
    renderHistory();
  });
  
  // 修改其他页面切换逻辑
  if (menuImage) {
    menuImage.addEventListener("click", () => {
      // 隐藏其他所有页面
      menuText.classList.remove("show");
      textPage.classList.remove("show");
      
      historyToggle.classList.remove("show");
      historyContent.classList.remove("show");
      clearHistoryBtn.classList.remove("show");
      
      menuSound.classList.remove("show");
      soundPage.classList.remove("show");
      
      // 显示背景页面
      menuImage.classList.add("show");
      imagePage.classList.add("show");
      
      try {
        // 判断updatePreviewBackground是否在当前脚本中定义
        if (typeof updatePreviewBackground === 'function') {
          updatePreviewBackground();
        } else {
          console.log("updatePreviewBackground函数未定义，尝试替代实现");
          
          // 内联实现背景预览更新
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
            
            console.log("内联实现 - 当前背景类:", currentBgClass, "路径:", backgroundImagePath);
            
            // 应用背景到预览区域
            kuosanPreview.style.backgroundImage = `url(${backgroundImagePath})`;
            kuosanPreview.style.backgroundSize = 'cover';
            kuosanPreview.style.backgroundPosition = 'center';
            
            // 同步背景到大预览区域
            document.documentElement.style.setProperty('--current-bg', `url(${backgroundImagePath})`);
          }
        }
      } catch (error) {
        console.error("更新预览背景时出错:", error);
      }
    });
  }
  
  if (menuSound) {
    menuSound.addEventListener("click", () => {
      // 隐藏其他所有页面
      menuText.classList.remove("show");
      textPage.classList.remove("show");
      
      historyToggle.classList.remove("show");
      historyContent.classList.remove("show");
      clearHistoryBtn.classList.remove("show");
      
      menuImage.classList.remove("show");
      imagePage.classList.remove("show");
      
      // 显示声音页面
      menuSound.classList.add("show");
      soundPage.classList.add("show");
    });
  }

  // 文字速度调节
  const speedInput = document.getElementById('speed-option');
  let setingspeed = false;
  if (speedInput) {
    speedInput.addEventListener("change", function () {
      // 保存选择
      const speed = this.value;
      localStorage.setItem("numSpeed", speed);

      // 应用速度设置（这里需要根据你的实际需求实现）
      applyTextSpeed(speed);
      setingspeed = true;
    });
  }
  
  // 定义全局变量numSpeed
  let numSpeed = 50;
  
  // 文字速度测试
  function testTextSpeed() {
    setingspeed = false;
    // 清空原始内容
    const Message = "钦灵Chat，测试文本显示速度";
    let i = 0;
    testMessage.textContent = ""; // 清空内容
    
    // 创建或获取光标元素
    let cursor = document.createElement("span");
    cursor.className = "typing-cursor";
    cursor.style.borderRight = "3px solid #f0ffff"; // 使用亮白色
    cursor.style.marginLeft = "2px";
    cursor.style.animation = "cursor-blink 0.7s infinite";
    cursor.style.height = "1.5em"; // 调整光标高度匹配新字体大小
    cursor.style.boxShadow = "none"; // 移除发光效果
    cursor.innerHTML = "&nbsp;";
    
    // 添加光标闪烁动画样式
    if (!document.getElementById("cursor-style")) {
      const style = document.createElement("style");
      style.id = "cursor-style";
      style.innerHTML = `
        @keyframes cursor-blink {
          0%, 100% { opacity: 1; }
          50% { opacity: 0; }
        }
      `;
      document.head.appendChild(style);
    }
    
    // 创建一个容器来存放打字内容和光标
    const container = document.createElement("div");
    container.style.display = "inline-block";
    container.style.fontFamily = "'Microsoft YaHei', '微软雅黑', sans-serif"; // 直接应用字体样式
    container.style.fontSize = "1.5em"; // 更大字号
    container.style.color = "#f0ffff"; // 使用亮白色
    container.style.textShadow = "none"; // 移除发光效果
    container.style.letterSpacing = "0.02em"; // 减小字间距
    container.style.wordSpacing = "0.05em"; // 减小词间距
    container.style.lineHeight = "1.4"; // 稍微减小行高
    container.style.verticalAlign = "bottom"; // 确保垂直对齐在底部
    container.appendChild(cursor);
    testMessage.appendChild(container);
    
    // 计算基础延迟时间
    const baseDelay = 200 - numSpeed;
    
    // 随机化打字速度函数，使打字效果更自然
    function getRandomDelay() {
      // 在基础延迟的80%-120%之间随机变化
      return baseDelay * (0.8 + Math.random() * 0.4);
    }
    
    // 定义打字函数
    function typeNextChar() {
      if (i < Message.length) {
        // 创建一个字符元素
        const charSpan = document.createElement("span");
        charSpan.textContent = Message.charAt(i);
        
        // 设置初始样式（为动画准备）
        charSpan.style.opacity = "0";
        charSpan.style.position = "relative";
        charSpan.style.top = "3px";
        charSpan.style.textShadow = "none"; // 移除发光效果
        
        // 插入到光标之前
        container.insertBefore(charSpan, cursor);
        
        // 触发淡入动画
        setTimeout(() => {
          charSpan.style.transition = "all 0.15s ease";
          charSpan.style.opacity = "1";
          charSpan.style.top = "0";
          charSpan.style.textShadow = "none"; // 保持无发光效果
          
          // 字符出现后，仍然保持无发光效果
          setTimeout(() => {
            charSpan.style.transition = "none";
            charSpan.style.textShadow = "none";
          }, 150);
        }, 10);
        
        i++;
        
        if (setingspeed) {
          clearTimeout(typeTimer); // 中断打字
          testTextSpeed(); // 重新开始
          return;
        }
        
        // 下一个字符使用随机延迟
        typeTimer = setTimeout(typeNextChar, getRandomDelay());
      } else {
        // 完成所有字符打字后，短暂暂停，然后重新开始
        setTimeout(() => {
          testTextSpeed();
        }, 1000);
      }
    }
    
    // 开始打字，添加短暂延迟模拟真实打字前的思考
    let typeTimer = setTimeout(typeNextChar, 300);
  }

  // 背景选择
  document.querySelectorAll(".bg-option").forEach((option) => {
    option.addEventListener("click", function () {
      // 移除所有active类
      document.querySelectorAll(".bg-option").forEach((bg) => {
        bg.classList.remove("active");
      });

      // 添加active类到当前选项
      this.classList.add("active");

      // 获取选择的背景
      const bgClass = this.dataset.bg;

      // 移除所有背景类
      document.body.className = "";

      // 添加选择的背景类
      document.body.classList.add(`bg-${bgClass}`);

      // 更新圣光显示样本的背景
      const kuosanTest = document.getElementById("kuosan-test");
      if (kuosanTest) {
        if (bgClass === "default") {
          kuosanTest.style.setProperty("--current-bg", "url(../pictures/backgrounds/homepage_bg.jpeg)");
        } else if (bgClass === "night") {
          kuosanTest.style.setProperty("--current-bg", "url(../pictures/backgrounds/homepage_bg2.jpg)");
        } else if (bgClass === "white") {
          kuosanTest.style.setProperty("--current-bg", "none");
          kuosanTest.style.backgroundColor = "#fff";
        }
      }

      // 保存选择
      localStorage.setItem("background", bgClass);
    });
  });

  // 从后端获取当前使用的AI模型信息
  function fetchCurrentModel() {
    const modelStatus = document.getElementById('model-status');
    if (modelStatus) {
      modelStatus.textContent = "正在获取模型信息...";
      modelStatus.className = "loading";
    }
    
    return fetch('/api/model')
      .then(response => {
        if (!response.ok) {
          throw new Error(`HTTP error! Status: ${response.status}`);
        }
        return response.json();
      })
      .then(data => {
        let modelName = '';
        
        // 尝试从不同的可能格式中提取模型名称
        if (data && data.model) {
          modelName = data.model;
          console.log('从API响应中获取模型名称:', modelName);
        } else if (data && typeof data === 'string') {
          // 处理纯文本响应
          modelName = data;
          console.log('从API响应中获取模型名称(字符串):', modelName);
        } else if (data && typeof data === 'object') {
          // 尝试从其他可能的字段中提取
          const possibleFields = ['name', 'modelName', 'model_name', 'type', 'modelType'];
          for (const field of possibleFields) {
            if (data[field]) {
              modelName = data[field];
              console.log(`从API响应的${field}字段中获取模型名称:`, modelName);
              break;
            }
          }
        }
        
        // 检查是否包含model="xxx"格式的字符串
        if (typeof modelName === 'string') {
          const modelMatch = modelName.match(/model\s*=\s*['"](.*?)['"]/);
          if (modelMatch && modelMatch[1]) {
            modelName = modelMatch[1];
            console.log('从model="xxx"格式中提取模型名称:', modelName);
          }
        }
        
        if (!modelName) {
          // 如果无法提取，使用一个默认值
          modelName = 'deepseek-ai/DeepSeek-V3';
          console.log('无法从API响应中提取模型名称，使用默认值:', modelName);
        }
        
        // 保存完整模型名称
        localStorage.setItem('currentAIModel', modelName);
        
        // 更新显示
        updateModelDisplay(modelName);
        return modelName;
      })
      .catch(error => {
        console.error('Error fetching model:', error);
        // 获取上次已知的模型（如果有）
        const lastModel = localStorage.getItem('currentAIModel') || 'deepseek-ai/DeepSeek-V3';
        
        // 更新状态为错误
        if (modelStatus) {
          modelStatus.textContent = `模型信息获取失败 (${getDisplayModelName(lastModel)})`;
          modelStatus.className = "error";
        }
        
        // 使用上次的模型信息更新显示
        updateModelDisplay(lastModel);
        return lastModel;
      });
  }

  // 获取模型的显示名称
  function getDisplayModelName(modelName) {
    // 提取模型名称的主要部分，使其更易读
    if (!modelName) return "未知模型";
    
    // 处理常见格式如 "organization/model-name"
    if (modelName.includes('/')) {
      const parts = modelName.split('/');
      return parts[parts.length - 1]; // 取最后一部分
    }
    
    return modelName;
  }
  
  // 检查并显示AI模型对应的logo
  function displayModelLogo(modelName) {
    if (!modelName) {
      console.log('无法显示logo：模型名称为空');
      return false;
    }
    
    console.log('正在查找模型logo，模型名称:', modelName);
    
    // 获取简化的模型名称用于匹配logo文件
    const simplifiedName = getDisplayModelName(modelName).toLowerCase();
    console.log('处理后的简化模型名称:', simplifiedName);
    
    // 创建一个img元素来测试logo文件是否存在
    const logoImg = new Image();
    let logoExists = false;
    
    // 保存所有尝试过的路径，用于调试
    const attemptedPaths = [];
    
    // 可能的logo文件路径格式
    const possibleFormats = [
      `../pictures/ai logo/${simplifiedName}.png`,
      `../pictures/ai logo/${simplifiedName}.jpg`,
      `../pictures/ai logo/${simplifiedName}.webp`,
      `../pictures/ai logo/${simplifiedName}.svg`
    ];
    attemptedPaths.push(...possibleFormats);
    
    // 也尝试空格替换为连字符的版本
    if (simplifiedName.includes(' ')) {
      const nameWithHyphens = simplifiedName.replace(/ /g, '-');
      possibleFormats.push(
        `../pictures/ai logo/${nameWithHyphens}.png`,
        `../pictures/ai logo/${nameWithHyphens}.jpg`,
        `../pictures/ai logo/${nameWithHyphens}.webp`,
        `../pictures/ai logo/${nameWithHyphens}.svg`
      );
      attemptedPaths.push(...possibleFormats.slice(-4));
    }
    
    // 也尝试完整名称（如果包含组织前缀）
    if (modelName.includes('/')) {
      const fullNameWithoutSlash = modelName.replace('/', '-').toLowerCase();
      possibleFormats.push(
        `../pictures/ai logo/${fullNameWithoutSlash}.png`,
        `../pictures/ai logo/${fullNameWithoutSlash}.jpg`,
        `../pictures/ai logo/${fullNameWithoutSlash}.webp`,
        `../pictures/ai logo/${fullNameWithoutSlash}.svg`
      );
      attemptedPaths.push(...possibleFormats.slice(-4));
    }
    
    // 处理版本号
    if (/\d+\.\d+/.test(simplifiedName)) {
      // 例如 gpt-3.5 -> gpt-3-5, gpt-4o -> gpt-4-o
      const normalizeName = simplifiedName.replace(/[.-]/g, '-');
      possibleFormats.push(
        `../pictures/ai logo/${normalizeName}.png`,
        `../pictures/ai logo/${normalizeName}.jpg`,
        `../pictures/ai logo/${normalizeName}.webp`,
        `../pictures/ai logo/${normalizeName}.svg`
      );
      attemptedPaths.push(...possibleFormats.slice(-4));
    }
    
    // 尝试一些常见的特殊模型名称转换
    const specialMappings = {
      'gpt-4': ['gpt4', 'chatgpt4', 'gpt', 'chatgpt'],
      'gpt-3.5': ['gpt3', 'gpt35', 'chatgpt', 'chatgpt3', 'gpt', 'gpt-3'],
      'claude': ['anthropic', 'claude-instant', 'claude2'],
      'claude-3': ['claude3', 'claude-3-opus', 'claude-3-sonnet', 'anthropic'],
      'claude-3-opus': ['claude3', 'claude3-opus', 'opus', 'anthropic'],
      'claude-3-sonnet': ['claude3', 'claude3-sonnet', 'sonnet', 'anthropic'],
      'claude-3-haiku': ['claude3', 'claude3-haiku', 'haiku', 'anthropic'],
      'gemini': ['gemini-pro', 'google-gemini', 'google'],
      'gemini-pro': ['gemini', 'google-gemini', 'google'],
      'gemini-ultra': ['gemini', 'google-gemini', 'google'],
      'llama': ['llama2', 'llama3', 'meta-llama', 'meta'],
      'llama-2': ['llama', 'llama2', 'meta-llama', 'meta'],
      'llama-3': ['llama', 'llama3', 'meta-llama', 'meta'],
      'deepseek': ['deepseek-ai', 'deepseek-v3', 'deepseek-7b', 'deepseek-moe']
    };
    
    // 添加特殊映射的logo路径
    for (const [key, alternatives] of Object.entries(specialMappings)) {
      if (simplifiedName.includes(key) || alternatives.some(alt => simplifiedName.includes(alt))) {
        console.log(`模型 ${simplifiedName} 匹配特殊映射 ${key}`);
        // 先添加特殊映射的关键字本身
        possibleFormats.push(
          `../pictures/ai logo/${key}.png`,
          `../pictures/ai logo/${key}.jpg`,
          `../pictures/ai logo/${key}.webp`,
          `../pictures/ai logo/${key}.svg`
        );
        attemptedPaths.push(...possibleFormats.slice(-4));
        
        // 再添加替代名称
        alternatives.forEach(alt => {
          possibleFormats.push(
            `../pictures/ai logo/${alt}.png`,
            `../pictures/ai logo/${alt}.jpg`,
            `../pictures/ai logo/${alt}.webp`,
            `../pictures/ai logo/${alt}.svg`
          );
          attemptedPaths.push(...possibleFormats.slice(-4));
        });
      }
    }
    
    // 尝试一些通用的名称
    const genericNames = ['ai', 'bot', 'chatbot', 'llm', 'large-language-model'];
    genericNames.forEach(name => {
      possibleFormats.push(
        `../pictures/ai logo/${name}.png`,
        `../pictures/ai logo/${name}.jpg`,
        `../pictures/ai logo/${name}.webp`,
        `../pictures/ai logo/${name}.svg`
      );
      attemptedPaths.push(...possibleFormats.slice(-4));
    });
    
    // 获取model-info-display元素
    const modelInfoDisplay = document.getElementById('model-info-display');
    if (!modelInfoDisplay) {
      console.log('无法找到model-info-display元素');
      return false;
    }
    
    // 移除之前的logo（如果有）
    const existingLogo = modelInfoDisplay.querySelector('.model-logo');
    if (existingLogo) {
      existingLogo.remove();
    }
    
    // 打印所有将要尝试的路径
    console.log('将尝试以下路径查找logo:', possibleFormats);
    
    // 尝试加载可能的logo文件
    function tryNextLogo(index) {
      if (index >= possibleFormats.length) {
        console.log('没有找到匹配的AI模型logo，尝试过的路径:', attemptedPaths);
        // 使用默认logo
        useFallbackLogo();
        return;
      }
      
      logoImg.onload = function() {
        // Logo存在，创建并添加logo元素
        const logoElement = document.createElement('img');
        logoElement.src = logoImg.src;
        logoElement.alt = `${modelName} Logo`;
        logoElement.className = 'model-logo';
        logoElement.style.maxHeight = '24px';
        logoElement.style.maxWidth = '24px';
        logoElement.style.marginRight = '8px';
        logoElement.style.verticalAlign = 'middle';
        
        // 添加到模型信息显示区域
        const modelStatus = document.getElementById('model-status');
        if (modelStatus) {
          modelInfoDisplay.insertBefore(logoElement, modelStatus);
          console.log(`已显示模型 ${modelName} 的logo，路径: ${logoImg.src}`);
          logoExists = true;
        }
      };
      
      logoImg.onerror = function() {
        console.log(`未找到logo: ${possibleFormats[index]}`);
        // 尝试下一个可能的格式
        tryNextLogo(index + 1);
      };
      
      console.log(`正在尝试加载: ${possibleFormats[index]}`);
      logoImg.src = possibleFormats[index];
    }
    
    // 使用默认logo的函数
    function useFallbackLogo() {
      const defaultLogoPath = '../pictures/ai logo/default.png';
      console.log('使用默认logo:', defaultLogoPath);
      
      const logoElement = document.createElement('img');
      logoElement.src = defaultLogoPath;
      logoElement.alt = 'AI Model';
      logoElement.className = 'model-logo';
      logoElement.style.maxHeight = '24px';
      logoElement.style.maxWidth = '24px';
      logoElement.style.marginRight = '8px';
      logoElement.style.verticalAlign = 'middle';
      
      logoElement.onerror = function() {
        console.log('默认logo也未找到，不显示任何logo');
      };
      
      // 添加到模型信息显示区域
      const modelStatus = document.getElementById('model-status');
      if (modelStatus) {
        modelInfoDisplay.insertBefore(logoElement, modelStatus);
      }
    }
    
    // 开始尝试加载第一个可能的logo
    tryNextLogo(0);
    
    return logoExists;
  }

  // 更新所有模型显示
  function updateModelDisplay(modelName) {
    // 更新模型状态显示
    const modelStatus = document.getElementById('model-status');
    if (modelStatus) {
      modelStatus.className = "success";
      const displayName = getDisplayModelName(modelName);
      modelStatus.textContent = `当前模型: ${displayName}`;
      
      // 尝试显示模型logo
      displayModelLogo(modelName);
    }
    
    // 更新聊天界面的模型显示
    const elementsToUpdate = document.querySelectorAll('.chat-model');
    const displayName = getDisplayModelName(modelName);
    
    elementsToUpdate.forEach(element => {
      element.textContent = displayName;
    });
    
    // 更新角色副标题（如果存在）
    const characterSub = document.getElementById('character-sub');
    if (characterSub) {
      // 保存原始副标题
      if (!localStorage.getItem('originalCharacterSub')) {
        localStorage.setItem('originalCharacterSub', characterSub.textContent);
      }
      
      const originalSub = localStorage.getItem('originalCharacterSub') || characterSub.textContent;
      characterSub.textContent = `${originalSub} • ${displayName}`;
    }
  }

  // 初始化
  function initSettings() {
    // 文字速度
    const savedSpeed = localStorage.getItem("numSpeed") || "50";
    speedInput.value = savedSpeed;
    applyTextSpeed(savedSpeed);
    testTextSpeed();

    // AI模型设置 - 从后端获取
    const modelStatus = document.getElementById('model-status');
    const modelSelect = document.getElementById('ai-model-select');
    
    if (modelStatus) {
      // 隐藏选择器，只显示状态
      if (modelSelect) {
        modelSelect.style.display = 'none';
      }
      
      // 更新状态为加载中
      modelStatus.textContent = "正在获取模型信息...";
      modelStatus.className = "loading";
      
      // 获取并显示后端模型信息
      fetchCurrentModel();
    }

    // 背景
    const savedBg = localStorage.getItem("background") || "night";
    
    // 检查是否是自定义背景
    if (savedBg.startsWith("custom-")) {
      const customId = savedBg.replace("custom-", "");
      const customBgData = localStorage.getItem(`customBg_${customId}`);
      
      // 如果找到了自定义背景数据
      if (customBgData) {
        const bgData = JSON.parse(customBgData);
        
        // 应用自定义背景
        document.body.className = "";
        document.body.classList.add(`bg-${savedBg}`);
        
        // 直接设置背景图片
        document.body.style.backgroundImage = `url(${bgData.dataUrl})`;
        document.body.style.backgroundSize = 'cover';
        document.body.style.backgroundPosition = 'center';
        
        // 查找对应的背景选项
        const customBgOption = document.querySelector(`.bg-option[data-bg="${savedBg}"]`);
        if (customBgOption) {
          document.querySelectorAll(".bg-option").forEach(bg => bg.classList.remove("active"));
          customBgOption.classList.add("active");
        }
        
        // 更新圣光显示样本的背景
        const kuosanTest = document.getElementById("kuosan-test");
        if (kuosanTest) {
          kuosanTest.style.setProperty("--current-bg", `url(${bgData.dataUrl})`);
        }
      } else {
        // 如果找不到自定义背景数据，则使用默认背景
        document.querySelector(`.bg-option[data-bg="night"]`).classList.add("active");
        document.body.classList.add(`bg-night`);
      }
    } else {
      // 应用内置背景
      document.querySelector(`.bg-option[data-bg="${savedBg}"]`).classList.add("active");
    document.body.classList.add(`bg-${savedBg}`);
    
    // 更新圣光显示样本的背景
    const kuosanTest = document.getElementById("kuosan-test");
    if (kuosanTest) {
      if (savedBg === "default") {
        kuosanTest.style.setProperty("--current-bg", "url(../pictures/backgrounds/homepage_bg.jpeg)");
      } else if (savedBg === "night") {
        kuosanTest.style.setProperty("--current-bg", "url(../pictures/backgrounds/homepage_bg2.jpg)");
      } else if (savedBg === "white") {
        kuosanTest.style.setProperty("--current-bg", "none");
        kuosanTest.style.backgroundColor = "#fff";
        }
      }
    }
    
    // 动画开关
    const animCheckbox = document.getElementById('animation-toggle');
    if (animCheckbox) {
      animCheckbox.checked = enableAnimation;
      animCheckbox.addEventListener('change', function() {
        enableAnimation = this.checked;
        localStorage.setItem('enableAnimation', enableAnimation);
        
        // 更新动画状态
        if (!enableAnimation) {
          // 禁用动画，添加no-animation类
          document.body.classList.add('no-animation');
          
          // 保留圣光效果相关元素的过渡效果
          const qinling = document.getElementById('qinling');
          const qinlingtest = document.getElementById('qinlingtest');
          const kuosanPreviewImg = document.getElementById('kuosan-preview-img');
          
          // 应用保留的过渡效果
          if (qinling) qinling.style.transition = "filter 0.3s ease-in-out, transform 0.3s ease-in-out";
          if (qinlingtest) qinlingtest.style.transition = "filter 0.3s ease-in-out, transform 0.3s ease-in-out";
          if (kuosanPreviewImg) kuosanPreviewImg.style.transition = "filter 0.3s ease-in-out, transform 0.3s ease-in-out";
          
          // 清空动画容器
          if (webpContainer) {
            webpContainer.innerHTML = '<div class="no-animation-placeholder"></div>';
          }
        } else {
          // 启用动画，移除no-animation类
          document.body.classList.remove('no-animation');
          // 重置动画容器（下次动画播放时会创建）
          if (webpContainer) {
            webpContainer.innerHTML = '';
          }
        }
      });
    }
  }

  // 应用文字速度
  function applyTextSpeed(speed) {
    numSpeed = parseInt(speed);
    // 在这里更新talk.js中的速度设置
    // 值为0时对应200ms延迟（最慢），值为200时对应10ms延迟（最快）
    const actualDelay = 200 - numSpeed;
    setTextSpeed(actualDelay);
    console.log(`文字速度设置为: ${speed}，实际延迟: ${actualDelay}ms`);
  }

  // 添加缺失的updatePreviewBackground函数定义
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
        backgroundImagePath = '../pictures/backgrounds/homepage_bg.jpeg';
      } else if (currentBgClass.includes('bg-night')) {
        backgroundImagePath = '../pictures/backgrounds/homepage_bg2.jpg';
      } else if (currentBgClass.includes('bg-white')) {
        backgroundImagePath = '../pictures/backgrounds/纯白背景.png';
      }
      
      console.log("当前背景类:", currentBgClass, "路径:", backgroundImagePath);
      
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

  // 初始化
  initSettings();

  // 初始化聊天界面的模型显示 - 使用后端模型
  const savedBackendModel = localStorage.getItem('backendModel');
  if (savedBackendModel) {
    updateModelDisplay(savedBackendModel);
  }

});
