document.addEventListener("DOMContentLoaded", () => {
  const navMenu = document.getElementById("nav-menu");
  const settingsForm = document.getElementById("settings-form");
  const contentTitle = document.getElementById("content-title");
  const contentDescription = document.getElementById("content-description");
  const saveButton = document.getElementById("save-button");
  const saveStatus = document.getElementById("save-status");
  const loader = document.getElementById("loader");

  let configData = null;
  let currentSettings = {};

  function showLoader(show) {
    loader.style.display = show ? "flex" : "none";
  }

  // 渲染导航菜单
  function renderNavMenu() {
    navMenu.innerHTML = "";
    for (const category in configData) {
      const catDiv = document.createElement("div");
      catDiv.className = "category";
      catDiv.innerHTML = `<span>${category}</span>`;

      const subcategories = configData[category].subcategories;
      for (const subcategory in subcategories) {
        const subLink = document.createElement("a");
        subLink.href = "#";
        subLink.className = "subcategory";
        subLink.textContent = subcategory;
        subLink.dataset.category = category;
        subLink.dataset.subcategory = subcategory;
        catDiv.appendChild(subLink);
      }
      navMenu.appendChild(catDiv);
    }
  }

  // 渲染表单内容
  function renderForm(category, subcategory) {
    const subData = configData[category].subcategories[subcategory];
    contentTitle.textContent = subcategory;
    contentDescription.textContent =
      subData.description || `修改 ${subcategory} 的相关配置`;
    settingsForm.innerHTML = "";
    currentSettings = {};

    subData.settings.forEach((setting) => {
      currentSettings[setting.key] = setting.value;
      const group = document.createElement("div");
      group.className = "form-group";

      const { key, value, description, type } = setting;

      // --- 核心逻辑修改：使用 'type' 字段来决定控件类型 ---
      switch (type) {
        case "bool":
          group.innerHTML = `
                    <label class="checkbox-container">${key}
                        <input type="checkbox" id="${key}" data-key="${key}" ${
            value.toLowerCase() === "true" ? "checked" : ""
          }>
                        <span class="checkmark"></span>
                    </label>
                    <p class="description">${description || ""}</p>
                `;
          break;

        case "textarea":
          group.innerHTML = `
                    <label for="${key}">${key}</label>
                    <p class="description">${description || "支持多行输入"}</p>
                    <textarea id="${key}" data-key="${key}" class="form-control-textarea" rows="8">${value}</textarea>
                `;
          break;

        case "text":
        default:
          group.innerHTML = `
                    <label for="${key}">${key}</label>
                    <p class="description">${description || ""}</p>
                    <input type="text" id="${key}" data-key="${key}" value="${value}">
                `;
          break;
      }
      settingsForm.appendChild(group);
    });
  }

  // 导航点击事件
  navMenu.addEventListener("click", (e) => {
    if (e.target.classList.contains("subcategory")) {
      e.preventDefault();

      // 更新激活状态
      document
        .querySelectorAll(".subcategory.active")
        .forEach((el) => el.classList.remove("active"));
      e.target.classList.add("active");

      const category = e.target.dataset.category;
      const subcategory = e.target.dataset.subcategory;
      renderForm(category, subcategory);
    }
  });

  // 保存按钮点击事件
  saveButton.addEventListener("click", async () => {
    const formData = {};

    // 从表单收集所有数据
    settingsForm.querySelectorAll("[data-key]").forEach((input) => {
      const key = input.dataset.key;
      if (input.type === "checkbox") {
        formData[key] = input.checked ? "true" : "false";
      } else {
        formData[key] = input.value;
      }
    });

    showLoader(true);
    saveStatus.textContent = "";

    try {
      const response = await fetch("/api/settings/config", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(formData),
      });

      const result = await response.json();

      if (!response.ok) {
        throw new Error(result.detail || "保存失败");
      }

      saveStatus.textContent = result.message;
      saveStatus.style.color = "var(--success-color)";

      // 重新加载配置以同步UI
      await loadConfig(false);
      // 重新渲染当前视图
      const activeLink = document.querySelector(".subcategory.active");
      if (activeLink) {
        renderForm(activeLink.dataset.category, activeLink.dataset.subcategory);
      }
    } catch (error) {
      saveStatus.textContent = `错误: ${error.message}`;
      saveStatus.style.color = "red";
    } finally {
      showLoader(false);
      setTimeout(() => {
        saveStatus.textContent = "";
      }, 5000);
    }
  });

  // 初始加载配置
  async function loadConfig(selectFirst = true) {
    showLoader(true);
    try {
      const response = await fetch("/api/settings/config");
      if (!response.ok) throw new Error("无法加载配置");
      configData = await response.json();
      renderNavMenu();

      if (selectFirst) {
        // 默认选中并显示第一个配置项
        const firstSubLink = navMenu.querySelector(".subcategory");
        if (firstSubLink) {
          firstSubLink.click();
        }
      }
    } catch (error) {
      console.error(error);
      settingsForm.innerHTML = `<p style="color: red;">加载配置失败: ${error.message}</p>`;
    } finally {
      showLoader(false);
    }
  }

  loadConfig();
});
