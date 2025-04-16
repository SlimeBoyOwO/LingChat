// 菜单功能实现
document.addEventListener("DOMContentLoaded", function () {
  const menuToggle = document.getElementById("menu-toggle");
  const menuContent = document.getElementById("menu-content");
  const menuText = document.getElementById("menu-text");
  const textPage = document.getElementById("text-page");
  const closeMenu = document.getElementById("close-menu");
  const testMessage = document.getElementById("testmessage");

  // 切换显示/隐藏
  menuToggle.addEventListener("click", () => {
    menuContent.classList.add("show");
    menuText.classList.add("show");
    textPage.classList.add("show");
    if (menuContent.classList.add("show")) {
      renderHistory();
    }
  });

    // 切换显示/隐藏
    menuText.addEventListener("click", () => {
      menuText.classList.add("show");
      textPage.classList.add("show");
      historyToggle.classList.remove("show");
      historyContent.classList.remove("show");
      clearHistoryBtn.classList.remove("show");
      menuImage.classList.remove("show");
      imagePage.classList.remove("show");
      menuSound.classList.remove("show");
      soundPage.classList.remove("show");
      if (menuContent.classList.add("show")) {
        renderHistory();
      }
    });

  // 切换显示/隐藏
  closeMenu.addEventListener("click", () => {
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
    if (menuContent.classList.remove("show")) {
      renderHistory();
    }
  });

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
  
  //文字速度测试
  function testTextSpeed() {
    setingspeed = false;
    //清空原始内容
    Message = "钦灵 Chat，测试文本显示速度";
    let i = 0;
    testMessage.textContent = ""; // 清空内容
    const timer = setInterval(() => {
      if (i < Message.length) {
        // 逐个字符追加（保留换行和空格）
        testMessage.textContent += Message.charAt(i);
        i++;
        if(setingspeed){
          clearInterval(timer) //提前打断动画
          testTextSpeed();
        }
      } else {
        clearInterval(timer); // 动画结束
        restartTimer = setTimeout(() => {
          testTextSpeed();
      }, 500);
      }
    }, numSpeed);
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

      // 保存选择
      localStorage.setItem("background", bgClass);
    });
  });

  // 初始化设置
  function initSettings() {
    // 文字速度
    const savedSpeed = localStorage.getItem("numSpeed") || "50";
    speedInput.value = savedSpeed;
    applyTextSpeed(savedSpeed);
    testTextSpeed();

    // 背景
    const savedBg = localStorage.getItem("background") || "night";
    document
      .querySelector(`.bg-option[data-bg="${savedBg}"]`)
      .classList.add("active");
    document.body.classList.add(`bg-${savedBg}`);
  }

  // 应用文字速度（示例函数，需要根据你的实际需求修改）
  function applyTextSpeed(speed) {
    console.log(`文字速度设置为: ${speed}`);
    setTextSpeed(speed);
  }

  // 初始化
  initSettings();

});
