// 菜单功能实现
document.addEventListener("DOMContentLoaded", function () {
  const menuToggle = document.getElementById("menu-toggle");
  const menuContent = document.getElementById("menu-content");
  const closeMenu = document.getElementById("close-menu");

  // 切换显示/隐藏
  menuToggle.addEventListener("click", () => {
    menuContent.classList.toggle("show");
    if (menuContent.classList.contains("show")) {
      renderHistory();
    }
  });

  // 切换显示/隐藏
  closeMenu.addEventListener("click", () => {
    menuContent.classList.toggle("show");
    if (menuContent.classList.contains("show")) {
      renderHistory();
    }
  });

  // 点击外部关闭
  document.addEventListener("click", (e) => {
    if (!menuContent.contains(e.target) && e.target !== menuToggle) {
      menuContent.classList.remove("show");
    }
  });

  // 文字速度选择
  document.querySelectorAll(".speed-option").forEach((option) => {
    option.addEventListener("click", function () {
      // 移除所有active类
      document.querySelectorAll(".speed-option").forEach((btn) => {
        btn.classList.remove("active");
      });

      // 添加active类到当前选项
      this.classList.add("active");

      // 保存选择
      const speed = this.dataset.speed;
      localStorage.setItem("textSpeed", speed);

      // 应用速度设置（这里需要根据你的实际需求实现）
      applyTextSpeed(speed);
    });
  });

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
    const savedSpeed = localStorage.getItem("textSpeed") || "medium";
    document
      .querySelector(`.speed-option[data-speed="${savedSpeed}"]`)
      .classList.add("active");
    applyTextSpeed(savedSpeed);

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
