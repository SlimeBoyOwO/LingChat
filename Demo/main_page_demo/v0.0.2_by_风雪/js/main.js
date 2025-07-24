document.addEventListener('DOMContentLoaded', () => {
    // --- DOM 元素获取 ---
    const settingsButton = document.getElementById('settings-button');
    const mainContainer = document.querySelector('.main-container');
    
    // 获取其他菜单按钮
    const menuButtons = document.querySelectorAll('.menu-button');
    const startGameButton = menuButtons[1]; // 第二个按钮：开始游戏
    const loadSaveButton = menuButtons[2];  // 第三个按钮：存档

    let activePanelId = null; // 跟踪当前打开的面板ID

    // --- 通用面板管理系统 ---
    
    /**
     * 打开指定ID的面板
     * @param {string} panelId 要打开的面板的ID (如 'settings-panel', 'game-screen-panel')
     */
    const openPanel = (panelId) => {
        // 清理所有已激活的面板类
        if (activePanelId) {
            const oldPanelType = activePanelId.replace('-panel', '');
            document.body.classList.remove('panel-active', `show-${oldPanelType}`);
        }

        // 添加通用激活类和新面板的特定类
        let panelType;
        if (panelId === 'settings-panel') {
            panelType = 'settings';
        } else if (panelId === 'game-screen-panel') {
            panelType = 'game-screen';
        } else if (panelId === 'load-save-panel') {
            panelType = 'load-save';
        } else {
            panelType = panelId.replace('-panel', '');
        }
        
        document.body.classList.add('panel-active', `show-${panelType}`);
        activePanelId = panelId;

        // 如果是设置面板，确保其内部交互只初始化一次
        if (panelId === 'settings-panel') {
            const settingsPanel = document.getElementById('settings-panel');
            if (settingsPanel && !settingsPanel.dataset.initialized) {
                setupSettingsInteraction();
                settingsPanel.dataset.initialized = 'true';
            }
        }
        // 对于其他面板，也可以在这里添加初始化逻辑
        // 例如： if (panelId === 'game-screen-panel') { initializeGameScreen(); }
    };
    
    /**
     * 关闭当前激活的面板
     */
    const closePanel = () => {
        if (activePanelId) {
            let panelType;
            if (activePanelId === 'settings-panel') {
                panelType = 'settings';
            } else if (activePanelId === 'game-screen-panel') {
                panelType = 'game-screen';
            } else if (activePanelId === 'load-save-panel') {
                panelType = 'load-save';
            } else {
                panelType = activePanelId.replace('-panel', '');
            }
            
            document.body.classList.remove('panel-active', `show-${panelType}`);
            activePanelId = null;
        }
    };
    
    // --- 菜单按钮事件绑定 ---
    
    // "设置"按钮点击事件
    settingsButton.addEventListener('click', (event) => {
        event.stopPropagation();
        openPanel('settings-panel');
    });

    // "开始游戏"按钮点击事件
    if (startGameButton) {
        startGameButton.addEventListener('click', (event) => {
            event.stopPropagation();
            openPanel('game-screen-panel');
        });
    }

    // "存档"按钮点击事件 (假设这是读取存档功能)
    if (loadSaveButton) {
        loadSaveButton.addEventListener('click', (event) => {
            event.stopPropagation();
            openPanel('load-save-panel');
        });
    }

    // 全局点击事件：处理"点击外部"或"关闭按钮"来隐藏面板
    document.addEventListener('click', (event) => {
        if (!document.body.classList.contains('panel-active') || !activePanelId) {
            return; // 如果没有面板激活，则不执行任何操作
        }

        const currentActivePanel = document.getElementById(activePanelId);
        if (!currentActivePanel) return;

        const clickInsidePanel = currentActivePanel.contains(event.target);
        // 使用 dataset 属性来标识关闭按钮属于哪个面板
        const clickedCloseButton = event.target.closest('[data-close-panel]');
        
        if (clickedCloseButton) {
            // 如果点击的是某个面板的关闭按钮，并且该按钮对应当前激活的面板
            if (clickedCloseButton.dataset.closePanel === activePanelId) {
                closePanel();
            }
        } else if (!clickInsidePanel) {
            // 如果点击在面板外部，则关闭面板
            closePanel();
        }
    });

    /**
     * @description 初始化设置面板内的所有交互，包括主导航、文本速度预览和高级设置的二级导航。
     * 该函数只应在面板首次打开时执行一次。
     */
    function setupSettingsInteraction() {
        // --- 元素获取 ---
        const settingsNav = document.querySelector('.settings-nav');
        if (!settingsNav) return; 

        const indicator = settingsNav.querySelector('.nav-indicator');
        const navButtons = settingsNav.querySelectorAll('.nav-button');
        const pages = document.querySelectorAll('.settings-page');
        const textSpeedSlider = document.getElementById('text-speed-slider');
        const textSampleDisplay = document.getElementById('typed-text-sample');

        // --- 文本速度预览逻辑 ---
        const sampleText = "Ling Chat：测试文本显示速度";
        let typingInterval;

        /**
         * @description 根据滑块速度值，模拟打字机效果。
         * @param {number} speed - 速度值 (1-100)，值越大速度越快。
         */
        function startTyping(speed) {
            clearInterval(typingInterval);
            if (!textSampleDisplay) return;

            textSampleDisplay.textContent = '';
            let i = 0;
            // 将滑块值 (1-100) 线性映射到打字延迟 (200ms - 10ms)。
            const maxDelay = 200;
            const minDelay = 10;
            const delay = maxDelay - ((speed - 1) / 99) * (maxDelay - minDelay);

            typingInterval = setInterval(() => {
                if (i < sampleText.length) {
                    textSampleDisplay.textContent += sampleText.charAt(i);
                    i++;
                } else {
                    clearInterval(typingInterval);
                }
            }, delay);
        }

        if (textSpeedSlider) {
            textSpeedSlider.addEventListener('input', (e) => startTyping(e.target.value));
            startTyping(textSpeedSlider.value); // 初始触发一次
        }

        // --- 主导航栏交互 (顶部) ---
        
        /**
         * @description 将顶部导航的指示器移动到目标按钮。
         * @param {HTMLElement} target - 目标按钮元素。
         */
        function moveIndicator(target) {
            if (!indicator || !target) return;
            indicator.style.left = `${target.offsetLeft}px`;
            indicator.style.width = `${target.offsetWidth}px`;
        }
        
        /**
         * @description 根据 data-content 属性切换显示的页面。
         * @param {string} targetContentId - 目标页面的ID。
         */
        function switchPage(targetContentId) {
            pages.forEach(page => {
                page.classList.toggle('active', page.id === targetContentId);
            });
        }

        navButtons.forEach(button => {
            button.addEventListener('click', (e) => {
                const currentActive = settingsNav.querySelector('.nav-button.active');
                if (currentActive) currentActive.classList.remove('active');
                
                const target = e.currentTarget;
                target.classList.add('active');
                
                moveIndicator(target);
                switchPage(target.dataset.content);
                
                // 如果切换到文本页面，重新触发打字效果
                if (target.dataset.content === 'text-settings' && textSpeedSlider) {
                    startTyping(textSpeedSlider.value);
                }

                // 如果切换到高级设置页面，确保二级导航的指示器位置正确
                if (target.dataset.content === 'advanced-settings') {
                    const advNav = document.querySelector('.advanced-nav');
                    if (advNav) {
                        const initialActiveAdvLink = advNav.querySelector('.adv-nav-link.active');
                        if (initialActiveAdvLink) {
                            moveAdvIndicator(initialActiveAdvLink);
                        }
                    }
                }
            });
        });

        const initialActiveButton = settingsNav.querySelector('.nav-button.active');
        if (initialActiveButton) {
            moveIndicator(initialActiveButton);
        }

        // --- 高级设置页面内部的二级导航逻辑 ---
        const advancedNav = document.querySelector('.advanced-nav');
        if (advancedNav) {
            const advIndicator = advancedNav.querySelector('.adv-nav-indicator');
            const advNavLinks = advancedNav.querySelectorAll('.adv-nav-link');
            const advContentPages = document.querySelectorAll('.adv-content-page');

            /**
             * @description 将高级设置的二级导航指示器移动到目标链接。
             * @param {HTMLElement} target - 目标链接元素。
             */
            function moveAdvIndicator(target) {
                if (!advIndicator || !target) return;
                advIndicator.style.top = `${target.offsetTop}px`;
                advIndicator.style.height = `${target.offsetHeight}px`;
            }

            advNavLinks.forEach(link => {
                link.addEventListener('click', (e) => {
                    e.preventDefault();

                    const currentActiveLink = advancedNav.querySelector('.adv-nav-link.active');
                    if (currentActiveLink) currentActiveLink.classList.remove('active');
                    
                    const targetLink = e.currentTarget;
                    targetLink.classList.add('active');
                    moveAdvIndicator(targetLink);

                    const targetContentId = targetLink.dataset.content;
                    advContentPages.forEach(page => {
                        page.classList.toggle('active', page.id === targetContentId);
                    });
                });
            });

            // 初始化二级导航指示器位置
            const initialActiveAdvLink = advancedNav.querySelector('.adv-nav-link.active');
            if (initialActiveAdvLink && advIndicator) {
                advIndicator.classList.add('no-transition');
                moveAdvIndicator(initialActiveAdvLink);
                setTimeout(() => {
                    advIndicator.classList.remove('no-transition');
                }, 0);
            }
        }
    }
}); 