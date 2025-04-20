/**
 * 统一高性能菜单系统 - 完全无动画版本
 * 整合了所有菜单功能，优化性能，移除所有动画效果
 */

// 全局菜单状态
const MenuSystem = {
    // 菜单元素引用
    elements: {},
    
    // 菜单状态
    state: {
        isMenuOpen: false,
        currentTab: null,
        isSettingPage: false,
        isBackgroundPage: false,
        pageTransitionActive: false
    },
    
    // 菜单配置
    settings: {
        textSpeed: 50,
        enableAnimation: false,
        background: 'default',
        enableHalo: true,
        kuosanValue: 18,
        audioVolume: 100
    },
    
    /**
     * 初始化菜单系统
     */
    init: function() {
        console.log('统一菜单系统初始化开始...');
        
        // 立即添加无动画类到body
        document.body.classList.add('no-animation');
        
        // 获取所有菜单元素
        this.cacheElements();
        
        // 加载保存的设置
        this.loadSettings();
        
        // 创建页面过渡动画元素
        this.createPageTransition();
        
        // 应用初始设置
        this.applySettings();
        
        // 初始化菜单UI状态
        this.initializeUIState();
        
        // 绑定事件处理器
        this.bindEvents();
        
        console.log('统一菜单系统初始化完成');
    },
    
    /**
     * 创建页面过渡动画元素
     */
    createPageTransition: function() {
        // 创建页面切换动画元素
        const pageTransition = document.createElement("div");
        pageTransition.className = "page-transition";
        
        // 使用WebP序列帧代替单个WebP动画
        if (this.settings.enableAnimation) {
            pageTransition.innerHTML = `<div class="webp-animation-container"></div>`;
            this.elements.webpContainer = pageTransition.querySelector(".webp-animation-container");
        } else {
            pageTransition.innerHTML = `<div class="no-animation-placeholder"></div>`;
        }
        
        document.body.appendChild(pageTransition);
        
        // 存储页面过渡元素
        this.elements.pageTransition = pageTransition;
    },
    
    /**
     * 播放页面过渡动画
     */
    playPageTransition: function(callback) {
        // 如果动画已禁用，直接执行回调
        if (!this.settings.enableAnimation) {
            if (callback && typeof callback === "function") {
                callback();
            }
            return;
        }
        
        // 防止重复触发动画
        if (this.state.pageTransitionActive) {
            if (callback && typeof callback === "function") {
                callback();
            }
            return;
        }
        
        this.state.pageTransitionActive = true;
        this.elements.pageTransition.classList.add("active");
        
        // 使用WebP帧序列创建动画
        if (this.elements.webpContainer) {
            this.elements.webpContainer.innerHTML = ''; // 清空容器
            
            // 播放WebP帧序列
            this.playWebpSequence(0, 39, 50, () => {
                this.elements.pageTransition.classList.remove("active");
                this.state.pageTransitionActive = false;
                
                if (callback && typeof callback === "function") {
                    callback();
                }
            });
        } else {
            // 如果没有WebP容器，使用计时器模拟动画
            setTimeout(() => {
                this.elements.pageTransition.classList.remove("active");
                this.state.pageTransitionActive = false;
                
                if (callback && typeof callback === "function") {
                    callback();
                }
            }, 1500); // 1.5秒延迟
        }
    },
    
    /**
     * 播放WebP帧序列动画
     * @param {number} startFrame - 起始帧
     * @param {number} endFrame - 结束帧
     * @param {number} frameDelay - 每帧间隔(毫秒)
     * @param {Function} callback - 动画结束回调
     */
    playWebpSequence: function(startFrame, endFrame, frameDelay, callback) {
        if (!this.elements.webpContainer) return;
        
        let currentFrame = startFrame;
        const img = document.createElement('img');
        img.className = 'webp-frame';
        img.style.width = '100%';
        img.style.height = '100%';
        img.style.objectFit = 'cover';
        
        this.elements.webpContainer.appendChild(img);
        
        // 预加载所有帧以确保动画流畅
        const preloadImages = [];
        for (let i = startFrame; i <= endFrame; i++) {
            const preloadImg = new Image();
            preloadImg.src = `../pictures/donghua/${i}.webp`;
            preloadImages.push(preloadImg);
        }
        
        // 播放动画帧序列
        const playFrame = () => {
            if (currentFrame <= endFrame) {
                img.src = `../pictures/donghua/${currentFrame}.webp`;
                currentFrame++;
                setTimeout(playFrame, frameDelay);
            } else {
                // 动画结束，执行回调
                if (callback && typeof callback === 'function') {
                    callback();
                }
            }
        };
        
        // 开始播放第一帧
        playFrame();
    },
    
    /**
     * 缓存所有菜单DOM元素引用
     */
    cacheElements: function() {
        // 主菜单元素
        this.elements.menuToggle = document.getElementById('menu-toggle');
        this.elements.menuContent = document.getElementById('menu-content');
        this.elements.closeMenu = document.getElementById('close-menu');
        
        // 菜单选项卡
        this.elements.menuText = document.getElementById('menu-text');
        this.elements.menuImage = document.getElementById('menu-image');
        this.elements.menuSound = document.getElementById('menu-sound');
        this.elements.menuHistory = document.getElementById('menu-history');
        
        // 页面内容
        this.elements.textPage = document.getElementById('text-page');
        this.elements.backgroundPage = document.getElementById('background-page');
        this.elements.soundPage = document.getElementById('sound-page');
        this.elements.historyPage = document.getElementById('history-page');
        
        // 设置控件
        this.elements.speedOption = document.getElementById('speed-option');
        this.elements.animationToggle = document.getElementById('animation-toggle');
        this.elements.bgOptions = document.querySelectorAll('.bg-option');
        this.elements.filterKuosan = document.getElementById('filter-kuosan');
        this.elements.haloToggle = document.getElementById('halo-toggle');
        this.elements.clearHistory = document.getElementById('clear-history');
        
        // 预览元素
        this.elements.kuosanPreviewImg = document.getElementById('kuosan-preview-img');
        this.elements.kuosanTest = document.getElementById('kuosan-test');
        this.elements.qinling = document.getElementById('qinling');
        this.elements.qinlingtest = document.getElementById('qinlingtest');
        
        // 状态显示
        this.elements.status = document.getElementById('status');
        this.elements.audioStatus = document.getElementById('audioStatus');
        
        console.log('菜单DOM元素缓存完成');
    },
    
    /**
     * 从本地存储加载保存的设置
     */
    loadSettings: function() {
        try {
            // 文本速度
            const savedTextSpeed = localStorage.getItem('textSpeed');
            if (savedTextSpeed) {
                this.settings.textSpeed = parseInt(savedTextSpeed);
            }
            
            // 动画设置
            const savedAnimation = localStorage.getItem('enableAnimation');
            if (savedAnimation !== null) {
                this.settings.enableAnimation = savedAnimation === 'true';
            }
            
            // 背景设置
            const savedBackground = localStorage.getItem('background');
            if (savedBackground) {
                this.settings.background = savedBackground;
            }
            
            // 圣光设置
            const savedHalo = localStorage.getItem('enableHalo');
            if (savedHalo !== null) {
                this.settings.enableHalo = savedHalo !== 'false';
            }
            
            // 圣光扩散值
            const savedKuosan = localStorage.getItem('Kuosan');
            if (savedKuosan) {
                this.settings.kuosanValue = parseInt(savedKuosan);
            }
            
            // 音量设置
            const savedVolume = localStorage.getItem('audioVolume');
            if (savedVolume) {
                this.settings.audioVolume = parseInt(savedVolume);
            }
            
            console.log('设置加载完成:', this.settings);
        } catch (error) {
            console.error('加载设置时出错:', error);
        }
    },
    
    /**
     * 应用所有保存的设置
     */
    applySettings: function() {
        try {
            // 应用文本速度
            if (this.elements.speedOption) {
                this.elements.speedOption.value = this.settings.textSpeed;
            }
            
            // 应用动画设置
            if (this.elements.animationToggle) {
                this.elements.animationToggle.checked = this.settings.enableAnimation;
            }
            
            // 应用背景设置
            this.applyBackground(this.settings.background, false);
            
            // 应用圣光设置
            if (this.elements.haloToggle) {
                this.elements.haloToggle.checked = this.settings.enableHalo;
            }
            
            // 应用圣光扩散值
            if (this.elements.filterKuosan) {
                this.elements.filterKuosan.value = this.settings.kuosanValue;
            }
            
            // 应用圣光效果
            this.applyKuosan(this.settings.kuosanValue);
            
            console.log('所有设置已应用');
        } catch (error) {
            console.error('应用设置时出错:', error);
        }
    },
    
    /**
     * 初始化菜单UI状态
     */
    initializeUIState: function() {
        try {
            // 确保菜单初始状态为关闭
            this.closeMenu(true);
            
            // 更新背景选项的选中状态
            this.updateBackgroundSelection();
            
            // 更新预览区域
            this.updatePreviewBackground();
            
            // 更新历史记录
            if (typeof renderHistory === 'function') {
                renderHistory();
            }
            
            // 禁用菜单相关元素的过渡效果
            this.disableAllTransitions();
            
            console.log('菜单UI状态初始化完成');
        } catch (error) {
            console.error('初始化UI状态时出错:', error);
        }
    },
    
    /**
     * 禁用所有元素的过渡和动画效果
     */
    disableAllTransitions: function() {
        // 禁用主要菜单元素的过渡
        const menuElements = [
            this.elements.menuToggle,
            this.elements.menuContent,
            this.elements.textPage,
            this.elements.backgroundPage,
            this.elements.soundPage,
            this.elements.historyPage,
            this.elements.kuosanTest,
            this.elements.kuosanPreviewImg
        ];
        
        // 应用无过渡样式
        menuElements.forEach(element => {
            if (element) {
                element.style.transition = 'none';
                element.style.animation = 'none';
            }
        });
        
        // 禁用所有子菜单项的过渡
        const subElements = document.querySelectorAll('.menu-option, .bg-option, .slider-container, .toggle-container');
        subElements.forEach(element => {
            element.style.transition = 'none';
            element.style.animation = 'none';
        });
        
        console.log('已禁用所有元素的过渡和动画效果');
    },
    
    /**
     * 绑定所有事件处理器
     */
    bindEvents: function() {
        console.log('绑定菜单事件...');
        
        // 菜单切换按钮
        if (this.elements.menuToggle) {
            this.elements.menuToggle.addEventListener('click', () => {
                this.toggleMenu();
            });
        }
        
        // 关闭菜单按钮
        if (this.elements.closeMenu) {
            this.elements.closeMenu.addEventListener('click', () => {
                this.closeMenu();
            });
        }
        
        // 菜单选项卡切换
        if (this.elements.menuText) {
            this.elements.menuText.addEventListener('click', () => {
                this.switchTab('text');
            });
        }
        
        if (this.elements.menuImage) {
            this.elements.menuImage.addEventListener('click', () => {
                this.switchTab('background');
            });
        }
        
        if (this.elements.menuSound) {
            this.elements.menuSound.addEventListener('click', () => {
                this.switchTab('sound');
            });
        }
        
        if (this.elements.menuHistory) {
            this.elements.menuHistory.addEventListener('click', () => {
                this.switchTab('history');
            });
        }
        
        // 设置控件事件
        this.bindSettingEvents();
        
        // 点击菜单外部关闭菜单
        document.addEventListener('click', e => {
            if (this.state.isMenuOpen && 
                this.elements.menuContent && 
                !this.elements.menuContent.contains(e.target) && 
                e.target !== this.elements.menuToggle) {
                this.closeMenu();
            }
        });
        
        console.log('菜单事件绑定完成');
    },
    
    /**
     * 绑定设置控件事件
     */
    bindSettingEvents: function() {
        // 文本速度滑块
        if (this.elements.speedOption) {
            this.elements.speedOption.addEventListener('change', () => {
                const speed = this.elements.speedOption.value;
                localStorage.setItem('textSpeed', speed);
                this.settings.textSpeed = parseInt(speed);
                console.log(`文本速度设置为: ${speed}`);
            });
        }
        
        // 动画开关
        if (this.elements.animationToggle) {
            this.elements.animationToggle.addEventListener('change', () => {
                const enableAnimation = this.elements.animationToggle.checked;
                localStorage.setItem('enableAnimation', enableAnimation);
                this.settings.enableAnimation = enableAnimation;
                console.log(`动画设置为: ${enableAnimation ? '启用' : '禁用'}`);
                
                // 如果禁用动画，强制添加无动画类
                if (!enableAnimation) {
                    document.body.classList.add('no-animation');
                    
                    // 更新过渡动画元素
                    if (this.elements.pageTransition) {
                        this.elements.pageTransition.innerHTML = `<div class="no-animation-placeholder"></div>`;
                        this.elements.webpContainer = null;
                    }
                } else {
                    document.body.classList.remove('no-animation');
                    
                    // 更新过渡动画元素
                    if (this.elements.pageTransition) {
                        this.elements.pageTransition.innerHTML = `<div class="webp-animation-container"></div>`;
                        this.elements.webpContainer = this.elements.pageTransition.querySelector(".webp-animation-container");
                    }
                }
            });
        }
        
        // 背景选项
        if (this.elements.bgOptions && this.elements.bgOptions.length > 0) {
            this.elements.bgOptions.forEach(option => {
                option.addEventListener('click', () => {
                    const bgType = option.getAttribute('data-bg');
                    if (bgType) {
                        this.applyBackground(bgType, false);
                        localStorage.setItem('background', bgType);
                        this.settings.background = bgType;
                        
                        // 更新选中状态
                        this.elements.bgOptions.forEach(opt => opt.classList.remove('active'));
                        option.classList.add('active');
                        
                        // 更新预览
                        this.updatePreviewBackground();
                    }
                });
            });
        }
        
        // 圣光扩散滑块
        if (this.elements.filterKuosan) {
            this.elements.filterKuosan.addEventListener('change', () => {
                const kuosanValue = this.elements.filterKuosan.value;
                localStorage.setItem('Kuosan', kuosanValue);
                this.settings.kuosanValue = parseInt(kuosanValue);
                console.log(`圣光扩散设置为: ${kuosanValue}`);
                this.applyKuosan(kuosanValue);
            });
        }
        
        // 圣光开关
        if (this.elements.haloToggle) {
            this.elements.haloToggle.addEventListener('change', () => {
                const enableHalo = this.elements.haloToggle.checked;
                localStorage.setItem('enableHalo', enableHalo);
                this.settings.enableHalo = enableHalo;
                console.log(`圣光效果设置为: ${enableHalo ? '启用' : '禁用'}`);
                this.applyKuosan(this.settings.kuosanValue);
                
                // 显示/隐藏滑块容器
                const kuosanContainer = this.elements.filterKuosan ? this.elements.filterKuosan.parentElement : null;
                if (kuosanContainer) {
                    if (enableHalo) {
                        kuosanContainer.style.width = '100%';
                        kuosanContainer.style.opacity = '1';
                        kuosanContainer.style.visibility = 'visible';
                    } else {
                        kuosanContainer.style.width = '0';
                        kuosanContainer.style.opacity = '0';
                        kuosanContainer.style.visibility = 'hidden';
                    }
                }
            });
        }
        
        // 清空历史按钮
        if (this.elements.clearHistory) {
            this.elements.clearHistory.addEventListener('click', () => {
                if (typeof clearHistory === 'function') {
                    clearHistory();
                } else {
                    localStorage.removeItem('chatHistory');
                    if (this.elements.historyPage) {
                        const historyList = this.elements.historyPage.querySelector('#history-list');
                        if (historyList) {
                            historyList.innerHTML = '<p>无历史记录</p>';
                        }
                    }
                }
                console.log('历史记录已清空');
            });
        }
    },
    
    /**
     * 切换菜单显示/隐藏
     */
    toggleMenu: function() {
        if (this.state.isMenuOpen) {
            this.closeMenu();
        } else {
            this.openMenu();
        }
    },
    
    /**
     * 打开菜单
     */
    openMenu: function() {
        if (this.elements.menuContent) {
            this.elements.menuContent.classList.add('show');
            this.state.isMenuOpen = true;
            
            // 默认显示文本标签页
            this.switchTab('text');
        }
    },
    
    /**
     * 关闭菜单
     */
    closeMenu: function() {
        if (this.elements.menuContent) {
            this.elements.menuContent.classList.remove('show');
            this.hideAllTabs();
            this.state.isMenuOpen = false;
        }
    },
    
    /**
     * 显示指定的选项卡
     */
    showTab: function(tabName) {
        try {
            console.log(`切换到${tabName}选项卡 - 使用WebP动画`);
            
            // 使用过渡动画
            this.playPageTransition(() => {
                // 隐藏所有选项卡内容
                const tabContents = [
                    this.elements.textPage,
                    this.elements.backgroundPage,
                    this.elements.soundPage,
                    this.elements.historyPage
                ];
                
                tabContents.forEach(tab => {
                    if (tab) {
                        tab.classList.remove('show');
                        tab.style.visibility = 'hidden';
                        tab.style.opacity = '0';
                    }
                });
                
                // 移除所有选项卡按钮的选中状态
                const tabButtons = [
                    this.elements.menuText,
                    this.elements.menuImage,
                    this.elements.menuSound,
                    this.elements.menuHistory
                ];
                
                tabButtons.forEach(button => {
                    if (button) {
                        button.classList.remove('show');
                    }
                });
                
                // 显示选中的选项卡内容
                let targetTab = null;
                let targetButton = null;
                
                switch (tabName) {
                    case 'text':
                        targetTab = this.elements.textPage;
                        targetButton = this.elements.menuText;
                        break;
                    case 'background':
                        targetTab = this.elements.backgroundPage;
                        targetButton = this.elements.menuImage;
                        // 更新预览背景
                        this.updatePreviewBackground();
                        break;
                    case 'sound':
                        targetTab = this.elements.soundPage;
                        targetButton = this.elements.menuSound;
                        break;
                    case 'history':
                        targetTab = this.elements.historyPage;
                        targetButton = this.elements.menuHistory;
                        break;
                    default:
                        console.error(`未知的选项卡: ${tabName}`);
                        return;
                }
                
                if (targetTab) {
                    targetTab.classList.add('show');
                    targetTab.style.visibility = 'visible';
                    targetTab.style.opacity = '1';
                }
                
                if (targetButton) {
                    targetButton.classList.add('show');
                }
                
                // 更新当前选项卡
                this.state.currentTab = tabName;
                
                console.log(`已切换到${tabName}选项卡`);
            });
        } catch (error) {
            console.error('切换选项卡时出错:', error);
        }
    },
    
    /**
     * 应用背景设置
     */
    applyBackground: function(type, withAnimation = false) {
        try {
            console.log(`设置背景: ${type}, 动画: ${withAnimation}`);
            
            // 获取聊天容器
            const chatContainer = document.querySelector('.chat-container');
            if (!chatContainer) return;
            
            // 重置背景类
            chatContainer.className = 'chat-container';
            
            // 应用背景类
            chatContainer.classList.add(`bg-${type}`);
            
            console.log(`背景已设置为: ${type}`);
        } catch (error) {
            console.error('设置背景时出错:', error);
        }
    },
    
    /**
     * 更新背景选项的选中状态
     */
    updateBackgroundSelection: function() {
        try {
            if (!this.elements.bgOptions || this.elements.bgOptions.length === 0) return;
            
            const currentBg = this.settings.background;
            
            // 更新选中状态
            this.elements.bgOptions.forEach(option => {
                const bgType = option.getAttribute('data-bg');
                if (bgType === currentBg) {
                    option.classList.add('active');
                } else {
                    option.classList.remove('active');
                }
            });
            
            console.log(`背景选择状态已更新: ${currentBg}`);
        } catch (error) {
            console.error('更新背景选择状态时出错:', error);
        }
    },
    
    /**
     * 更新预览区域的背景
     */
    updatePreviewBackground: function() {
        try {
            if (!this.elements.kuosanTest || !this.elements.kuosanPreviewImg) return;
            
            // 获取当前背景
            const currentBg = this.settings.background;
            
            // 背景图片URL
            let bgImage = '';
            
            // 根据当前背景获取对应的背景图片
            switch (currentBg) {
                case 'default':
                    bgImage = '../pictures/backgrounds/homepage_bg.jpeg';
                    break;
                case 'night':
                    bgImage = '../pictures/backgrounds/homepage_bg2.jpg';
                    break;
                case 'white':
                    bgImage = '../pictures/backgrounds/纯白背景.png';
                    break;
                default:
                    bgImage = '../pictures/backgrounds/homepage_bg.jpeg';
            }
            
            // 应用背景到预览区域
            document.documentElement.style.setProperty('--current-bg', `url(${bgImage})`);
            
            console.log(`预览背景已更新: ${currentBg}`);
        } catch (error) {
            console.error('更新预览背景时出错:', error);
        }
    },
    
    /**
     * 应用圣光扩散效果
     */
    applyKuosan: function(kuosanValue) {
        try {
            if (!this.elements.qinling || !this.elements.qinlingtest || !this.elements.kuosanPreviewImg) return;
            
            // 检查是否启用圣光效果
            const enableHalo = this.settings.enableHalo;
            
            if (enableHalo) {
                // 应用圣光滤镜
                const filterStyle = `drop-shadow(0 0 ${kuosanValue}px rgba(255, 255, 255, 1))`;
                this.elements.qinling.style.filter = filterStyle;
                this.elements.qinlingtest.style.filter = filterStyle;
                this.elements.kuosanPreviewImg.style.filter = filterStyle;
            } else {
                // 清除滤镜
                this.elements.qinling.style.filter = 'none';
                this.elements.qinlingtest.style.filter = 'none';
                this.elements.kuosanPreviewImg.style.filter = 'none';
            }
            
            console.log(`圣光效果已${enableHalo ? '应用' : '禁用'}, 扩散值: ${kuosanValue}`);
        } catch (error) {
            console.error('应用圣光效果时出错:', error);
        }
    },
    
    /**
     * 切换菜单选项卡
     * @param {string} tabName - 要切换到的选项卡名称
     */
    switchTab: function(tabName) {
        // 隐藏所有页面
        this.hideAllTabs();
        
        // 显示所选页面和按钮
        switch (tabName) {
            case 'text':
                if (this.elements.menuText) this.elements.menuText.classList.add('show');
                if (this.elements.textPage) {
                    this.elements.textPage.classList.add('show');
                    this.elements.textPage.style.display = 'block'; // 确保显示
                }
                break;
                
            case 'background':
                if (this.elements.menuImage) this.elements.menuImage.classList.add('show');
                if (this.elements.backgroundPage) {
                    this.elements.backgroundPage.classList.add('show');
                    this.elements.backgroundPage.style.display = 'block'; // 确保显示
                }
                this.updatePreviewBackground();
                break;
                
            case 'sound':
                if (this.elements.menuSound) this.elements.menuSound.classList.add('show');
                if (this.elements.soundPage) {
                    this.elements.soundPage.classList.add('show');
                    this.elements.soundPage.style.display = 'block'; // 确保显示
                }
                break;
                
            case 'history':
                if (this.elements.menuHistory) this.elements.menuHistory.classList.add('show');
                if (this.elements.historyPage) {
                    this.elements.historyPage.classList.add('show');
                    this.elements.historyPage.style.display = 'block'; // 确保显示
                    this.elements.historyPage.style.zIndex = '2'; // 确保在最上层
                }
                if (this.elements.clearHistory) this.elements.clearHistory.classList.add('show');
                this.renderHistory();
                break;
        }
        
        this.state.currentTab = tabName;
    },
    
    /**
     * 隐藏所有标签页
     */
    hideAllTabs: function() {
        // 移除所有菜单按钮的高亮状态
        if (this.elements.menuText) this.elements.menuText.classList.remove('show');
        if (this.elements.menuImage) this.elements.menuImage.classList.remove('show');
        if (this.elements.menuSound) this.elements.menuSound.classList.remove('show');
        if (this.elements.menuHistory) this.elements.menuHistory.classList.remove('show');
        
        // 隐藏所有页面内容
        if (this.elements.textPage) {
            this.elements.textPage.classList.remove('show');
            this.elements.textPage.style.display = 'none'; // 完全隐藏
        }
        if (this.elements.backgroundPage) {
            this.elements.backgroundPage.classList.remove('show');
            this.elements.backgroundPage.style.display = 'none'; // 完全隐藏
        }
        if (this.elements.soundPage) {
            this.elements.soundPage.classList.remove('show');
            this.elements.soundPage.style.display = 'none'; // 完全隐藏
        }
        if (this.elements.historyPage) {
            this.elements.historyPage.classList.remove('show');
            this.elements.historyPage.style.display = 'none'; // 完全隐藏
        }
        
        // 隐藏清除历史按钮
        if (this.elements.clearHistory) this.elements.clearHistory.classList.remove('show');
    },
    
    /**
     * 渲染历史记录
     */
    renderHistory: function() {
        if (typeof renderHistory === 'function') {
            renderHistory();
        }
    }
};

// 在页面加载时初始化菜单系统
document.addEventListener('DOMContentLoaded', function() {
    console.log('页面加载完成，初始化统一菜单系统...');
    MenuSystem.init();
    
    // 禁用ARONA动画，直接执行回调
    window.playAronaAnimation = function(callback) {
        console.log('跳过ARONA动画，直接执行回调');
        if (typeof callback === 'function') {
            try {
                callback();
            } catch (error) {
                console.error('执行ARONA动画回调时出错:', error);
            }
        }
    };
    
    // 将菜单系统暴露为全局变量
    window.MenuSystem = MenuSystem;
}); 