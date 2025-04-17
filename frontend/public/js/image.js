const qinling = document.getElementById("qinling");
const qinlingtest = document.getElementById("qinlingtest");
const filterKuosan = document.getElementById("filter-kuosan");

function initImage() {
    // 切换显示/隐藏
    menuImage.addEventListener("click", () => {
        menuImage.classList.add("show");
        imagePage.classList.add("show");
        historyToggle.classList.remove("show");
        historyContent.classList.remove("show");
        clearHistoryBtn.classList.remove("show");
        menuText.classList.remove("show");
        textPage.classList.remove("show");
        menuSound.classList.remove("show");
        soundPage.classList.remove("show");
        if (imagePage.classList.contains("show")) {
            renderHistory();
        }
    });

  if (filterKuosan) {
    filterKuosan.addEventListener("change", function () {
      // 保存选择
      const Kuosan = this.value;
      localStorage.setItem("Kuosan", Kuosan);
      // 应用设置
      console.log(`光晕扩散设置为: ${Kuosan}`);
      appleKuosan(Kuosan);
    });
  }

  function appleKuosan(Kuosan) {
    qinling.style.filter = `drop-shadow(0 0 ${Kuosan}px rgba(255, 255, 255, 1))`;
    qinlingtest.style.filter = `drop-shadow(0 0 ${Kuosan}px rgba(255, 255, 255, 1))`;
  }

  function initKuosan() {
    const Kuosan = localStorage.getItem("Kuosan") || "18";
    filterKuosan.value = Kuosan;
    appleKuosan(Kuosan);
  }

  initKuosan();
}

// 在页面加载时初始化
initImage();