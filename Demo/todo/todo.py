import webview
import json
import os
import datetime
import sys

# --- 配置文件路径 (增强版：兼容打包和开发环境) ---
if getattr(sys, 'frozen', False):
    # 如果是 PyInstaller 打包后的环境
    BASE_DIR = os.path.dirname(sys.executable)
else:
    # 开发环境
    BASE_DIR = os.path.dirname(os.path.abspath(__file__))

DATA_FILE = os.path.join(BASE_DIR, "todo_data.json")
LOG_FILE = os.path.join(BASE_DIR, "user_behavior.log")

# 打印路径以便调试 (请在控制台查看这个路径是否是你预期的位置)
print(f"[Debug] 数据文件路径: {DATA_FILE}")

# --- 默认数据 ---
DEFAULT_DATA = {
    "columns": [
        {
            "id": "col-1",
            "title": "今日待办",
            "collapsed": False,
            "tasks": [
                {
                    "id": "task-1",
                    "title": "体验 AI Todo",
                    "completed": False,
                    "dueDate": "",
                    "content": "尝试使用右侧番茄钟功能",
                    "steps": [
                        {"id": "s1", "text": "创建一个新任务", "completed": False},
                        {"id": "s2", "text": "选中它开始专注", "completed": False}
                    ]
                }
            ]
        }
    ],
    "settings": {
        "workTime": 25,
        "breakTime": 5,
        "cycles": 4
    }
}

class Api:
    def __init__(self):
        self.ensure_log_file()
        self.ensure_data_file()

    def ensure_log_file(self):
        if not os.path.exists(LOG_FILE):
            try:
                with open(LOG_FILE, 'w', encoding='utf-8') as f:
                    f.write(f"--- Session Start: {datetime.datetime.now()} ---\n")
            except Exception as e:
                print(f"[Error] Init log: {e}")

    def ensure_data_file(self):
        # 如果数据文件不存在，先写入默认数据，确保文件被创建
        if not os.path.exists(DATA_FILE):
            print("[Debug] 数据文件不存在，正在创建默认文件...")
            self.save_data(DEFAULT_DATA)

    def get_data(self):
        print(f"[Debug] get_data() 被调用")
        print(f"[Debug] 检查文件是否存在: {os.path.exists(DATA_FILE)}")
        if os.path.exists(DATA_FILE):
            try:
                with open(DATA_FILE, 'r', encoding='utf-8') as f:
                    content = f.read().strip()
                    print(f"[Debug] 文件内容长度: {len(content)} 字符")
                    if not content:
                        print("[Debug] 文件为空，返回默认数据")
                        return DEFAULT_DATA
                    
                    data = json.loads(content)
                    print(f"[Debug] 成功解析JSON，列数: {len(data.get('columns', []))}")
                    
                    # 兼容性处理：旧格式转新格式
                    if isinstance(data, list):
                        print("[Debug] 检测到旧数据格式，正在转换...")
                        return {
                            "columns": data,
                            "settings": DEFAULT_DATA["settings"]
                        }
                    print("[Debug] 返回数据到前端")
                    return data
            except Exception as e:
                print(f"[Error] 读取数据失败 (将使用默认数据): {e}")
                import traceback
                traceback.print_exc()
                return DEFAULT_DATA
        else:
            print("[Debug] 文件不存在，返回默认数据")
        return DEFAULT_DATA

    def save_data(self, data_input):
        try:
            content_to_write = ""
            
            # 关键修复：判断传入的是 字符串 还是 字典/对象
            # pywebview 有时会自动转换 JSON 字符串为 Python 字典
            if isinstance(data_input, (dict, list)):
                # 如果是对象，转为标准 JSON 字符串 (ensure_ascii=False 支持中文)
                content_to_write = json.dumps(data_input, ensure_ascii=False, indent=2)
            elif isinstance(data_input, str):
                # 如果是字符串，直接使用
                content_to_write = data_input
            else:
                # 其他情况强转字符串，防止崩溃
                content_to_write = str(data_input)

            with open(DATA_FILE, 'w', encoding='utf-8') as f:
                f.write(content_to_write)
            
            # print("[Debug] 数据保存成功") 
            return {"status": "success"}
        except Exception as e:
            print(f"[Error] 保存数据失败: {e}")
            return {"status": "error", "message": str(e)}

    def record_activity(self, action, content):
        now_str = datetime.datetime.now().strftime("%H:%M")
        log_entry = f"[{now_str}] {action}: {content}"
        print(f"[UserLog] {log_entry}")
        try:
            with open(LOG_FILE, "a", encoding="utf-8") as f:
                f.write(log_entry + "\n")
        except Exception as e:
            print(f"[Error] Writing log: {e}")
        return {"status": "logged"}

# --- 前端 HTML/JS ---
HTML_TEMPLATE = """
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AI Companion Todo</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/Sortable/1.15.0/Sortable.min.js"></script>
    
    <style>
        ::-webkit-scrollbar { width: 6px; height: 6px; }
        ::-webkit-scrollbar-track { background: transparent; }
        ::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.1); border-radius: 3px; }
        ::-webkit-scrollbar-thumb:hover { background: rgba(0, 0, 0, 0.2); }
        
        body {
            background-color: #f3f4f6;
            background-image: radial-gradient(#e5e7eb 1px, transparent 1px);
            background-size: 20px 20px;
        }
        
        [x-cloak] { display: none !important; }
        .sortable-ghost { opacity: 0.4; background: #cbd5e1; border: 1px dashed #94a3b8; }
        
        /* 圆形进度条样式 */
        .progress-ring__circle {
            transition: stroke-dashoffset 0.35s;
            transform: rotate(-90deg);
            transform-origin: 50% 50%;
        }
    </style>
</head>
<body class="h-screen w-screen overflow-hidden text-slate-800 font-sans antialiased">

    <!-- 主应用容器 -->
    <div x-data="todoApp()" x-init="initApp()" class="flex h-full w-full relative">
        
        <!-- 左侧：Todo 列表区域 (Flex 1) -->
        <div class="flex-1 flex flex-col p-4 md:p-6 overflow-hidden border-r border-slate-200">
            <header class="flex justify-between items-center mb-4 shrink-0">
                <h1 class="text-2xl font-bold text-slate-700 tracking-tight">工作台</h1>
                <button @click="addColumn" class="bg-white hover:bg-slate-50 border border-slate-200 text-slate-600 text-sm font-medium py-1.5 px-3 rounded shadow-sm transition">
                    + 新建清单
                </button>
            </header>

            <!-- 列表滚动区 -->
            <div id="columns-container" class="flex-1 overflow-y-auto pr-2 space-y-4 pb-10">
                <template x-for="(col, colIndex) in columns" :key="col.id">
                    <div class="column-item bg-white rounded-xl shadow-sm border border-slate-200 transition-all duration-200 group/col"
                         :class="{'opacity-80': col.collapsed}">
                        
                        <!-- 栏目头部 -->
                        <div class="flex items-center justify-between p-3 border-b border-slate-100 handle-col cursor-move">
                            <div class="flex items-center gap-2 flex-1">
                                <button @click="toggleColumnCollapse(col)" class="text-slate-400 hover:text-slate-600">
                                    <svg x-show="!col.collapsed" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                                    <svg x-show="col.collapsed" class="w-4 h-4 -rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                                </button>
                                <!-- 修改：栏目标题支持编辑日志 -->
                                <input type="text" x-model="col.title" 
                                       @focus="trackFieldEdit('col-' + col.id, col.title)"
                                       @blur="onFieldBlur('col-' + col.id, col.title, 'column-title')"
                                       class="bg-transparent font-bold text-slate-700 focus:outline-none focus:text-blue-600 w-full text-sm">
                            </div>
                            <button @click="deleteColumn(colIndex)" class="text-slate-300 hover:text-red-500 opacity-0 group-hover/col:opacity-100 transition px-2">×</button>
                        </div>

                        <!-- 任务列表 -->
                        <div x-show="!col.collapsed" :id="'task-list-' + colIndex" class="task-container p-2 space-y-1.5 min-h-[10px]">
                            <template x-for="(task, taskIndex) in col.tasks" :key="task.id">
                                <div class="relative group">
                                    <!-- 任务卡片 -->
                                    <div @click="openDetail(colIndex, taskIndex)" 
                                         class="task-item bg-slate-50 hover:bg-white border border-transparent hover:border-blue-200 hover:shadow-md rounded-lg p-2.5 cursor-pointer transition flex items-center gap-3">
                                        
                                        <!-- 选择模式下的复选框 (左侧) -->
                                        <div x-show="pomoMode === 'select'" @click.stop class="shrink-0" x-transition>
                                            <input type="checkbox" :checked="isTaskSelected(task.id)" @change="toggleSelectTask(task, col.title)"
                                                   class="w-4 h-4 text-indigo-500 rounded border-slate-300 focus:ring-indigo-500 cursor-pointer">
                                        </div>

                                        <!-- 完成状态 -->
                                        <div @click.stop="toggleTask(colIndex, taskIndex)" 
                                             class="w-5 h-5 rounded-full border-2 flex items-center justify-center cursor-pointer transition-all shrink-0"
                                             :class="task.completed ? 'bg-blue-500 border-blue-500' : 'border-slate-300 hover:border-blue-400 bg-white'">
                                            <svg x-show="task.completed" class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7"></path></svg>
                                        </div>

                                        <div class="flex-1 min-w-0">
                                            <p class="truncate text-sm font-medium text-slate-700 transition-all" 
                                               :class="{'line-through text-slate-400': task.completed}" 
                                               x-text="task.title"></p>
                                            <div x-show="task.steps.length > 0 || task.dueDate" class="flex gap-2 mt-0.5">
                                                <p x-show="task.dueDate" class="text-[10px] text-red-400 flex items-center gap-0.5">📅 <span x-text="task.dueDate"></span></p>
                                                <p x-show="task.steps.length > 0" class="text-[10px] text-slate-400 flex items-center gap-0.5">
                                                    <span>list:</span> 
                                                    <span x-text="task.steps.filter(s=>s.completed).length + '/' + task.steps.length"></span>
                                                </p>
                                            </div>
                                        </div>
                                        
                                        <div class="text-slate-300 opacity-0 group-hover:opacity-100 handle-task cursor-grab">
                                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8h16M4 16h16"></path></svg>
                                        </div>
                                    </div>
                                </div>
                            </template>
                        </div>
                        <div x-show="!col.collapsed" class="px-3 pb-2 pt-1">
                             <!-- 修复：支持 @blur 失去焦点时自动保存任务 -->
                             <input type="text" placeholder="+ 添加任务" 
                                    @keydown.enter="addTask($event, colIndex)"
                                    @blur="addTask($event, colIndex)"
                                    class="w-full bg-slate-50 hover:bg-slate-100 rounded px-2 py-1.5 text-sm placeholder-slate-400 focus:outline-none focus:ring-1 focus:ring-blue-300 transition">
                        </div>
                    </div>
                </template>
            </div>
        </div>

        <!-- 右侧：番茄钟面板 (W-80 / W-96) -->
        <div class="w-80 md:w-96 bg-white border-l border-slate-200 flex flex-col shadow-lg z-10 shrink-0">
            <!-- 头部 -->
            <div class="p-4 border-b border-slate-100 flex justify-between items-center">
                <h2 class="font-bold text-slate-700 flex items-center gap-2">
                    <span class="text-xl">🍅</span> 专注时刻
                </h2>
                <div class="text-xs font-mono text-slate-400 bg-slate-100 px-2 py-1 rounded">
                    Cycle: <span x-text="pomoState.cycleCount + 1"></span>/<span x-text="pomoSettings.cycles"></span>
                </div>
            </div>

            <!-- 中间：圆形计时器 -->
            <div class="flex-1 flex flex-col items-center justify-center p-6 relative">
                <!-- SVG Circle -->
                <div class="relative w-56 h-56 flex items-center justify-center">
                    <svg class="w-full h-full" viewBox="0 0 100 100">
                        <!-- 背景圆 -->
                        <circle cx="50" cy="50" r="45" fill="none" stroke="#f1f5f9" stroke-width="6" />
                        <!-- 进度圆 -->
                        <circle cx="50" cy="50" r="45" fill="none" 
                                :stroke="pomoState.status === 'break' ? '#10b981' : '#3b82f6'" 
                                stroke-width="6"
                                stroke-linecap="round"
                                class="progress-ring__circle"
                                :style="'stroke-dasharray: 283; stroke-dashoffset: ' + timeOffset" />
                    </svg>
                    <!-- 时间显示 -->
                    <div class="absolute inset-0 flex flex-col items-center justify-center text-slate-700">
                        <span class="text-5xl font-bold font-mono tracking-tighter" x-text="formatTime(pomoState.timeLeft)"></span>
                        <span class="text-xs uppercase font-bold tracking-widest mt-2 text-slate-400" 
                              x-text="pomoState.status === 'work' ? 'FOCUS' : (pomoState.status === 'break' ? 'BREAK' : 'IDLE')"></span>
                    </div>
                </div>

                <!-- 控制按钮 -->
                <div class="flex gap-4 mt-8">
                    <button @click="toggleTimer" 
                            class="w-14 h-14 rounded-full flex items-center justify-center text-white shadow-lg transition transform active:scale-95"
                            :class="pomoState.isRunning ? 'bg-amber-400 hover:bg-amber-500' : 'bg-blue-500 hover:bg-blue-600'">
                        <svg x-show="!pomoState.isRunning" class="w-6 h-6 ml-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"></path><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                        <svg x-show="pomoState.isRunning" class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                    </button>
                    <button @click="resetTimer" class="w-14 h-14 rounded-full bg-slate-100 hover:bg-slate-200 text-slate-500 flex items-center justify-center transition">
                        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path></svg>
                    </button>
                </div>
            </div>

            <!-- 底部：设置与意图 -->
            <div class="p-5 bg-slate-50 border-t border-slate-200">
                
                <!-- 模式切换 Tabs -->
                <div class="flex p-1 bg-slate-200/60 rounded-lg mb-4" x-show="pomoState.status === 'idle'">
                    <button @click="pomoMode = 'manual'; pomoQueue = []; logAction('POMO-MODE', '切换到自由输入模式')" 
                            class="flex-1 text-xs font-bold py-1.5 rounded-md transition"
                            :class="pomoMode === 'manual' ? 'bg-white text-blue-600 shadow-sm' : 'text-slate-500 hover:text-slate-600'">
                        自由输入
                    </button>
                    <button @click="pomoMode = 'select'; logAction('POMO-MODE', '切换到选择任务模式')" 
                            class="flex-1 text-xs font-bold py-1.5 rounded-md transition"
                            :class="pomoMode === 'select' ? 'bg-white text-indigo-600 shadow-sm' : 'text-slate-500 hover:text-slate-600'">
                        选择任务 <span x-show="pomoQueue.length > 0" class="ml-1 px-1.5 py-0.5 bg-indigo-100 text-indigo-600 rounded-full text-[10px]" x-text="pomoQueue.length"></span>
                    </button>
                </div>

                <!-- 模式内容 -->
                <div x-show="pomoState.status === 'idle'">
                    <!-- 自由输入模式 -->
                    <div x-show="pomoMode === 'manual'">
                        <textarea x-model="pomoManualInput" 
                                  class="w-full text-sm p-3 rounded border border-slate-200 focus:outline-none focus:border-blue-400 resize-none h-20 placeholder-slate-400 bg-white"
                                  placeholder="这次打算专注做什么？(例如：修复登录Bug)"></textarea>
                    </div>
                    
                    <!-- 任务选择模式提示 -->
                    <div x-show="pomoMode === 'select'" class="space-y-2">
                        <p class="text-xs text-slate-500 mb-2">在左侧列表中勾选任务或步骤：</p>
                        <div class="h-20 overflow-y-auto bg-white border border-slate-200 rounded p-2 space-y-1">
                            <template x-if="pomoQueue.length === 0">
                                <div class="text-center text-slate-400 text-xs py-4 italic">未选择任务</div>
                            </template>
                            <template x-for="(item, idx) in pomoQueue" :key="idx">
                                <div class="flex justify-between items-center text-xs text-slate-700 bg-slate-50 p-1.5 rounded border border-slate-100">
                                    <span class="truncate max-w-[180px]" x-text="item.title"></span>
                                    <button @click="removeFromQueue(idx)" class="text-slate-400 hover:text-red-500">×</button>
                                </div>
                            </template>
                        </div>
                    </div>
                </div>

                <!-- 运行时显示内容 -->
                <div x-show="pomoState.status !== 'idle'" class="bg-white p-3 rounded border border-slate-200 h-32 overflow-y-auto">
                    <p class="text-xs font-bold text-slate-400 uppercase mb-1">当前任务</p>
                    <template x-if="pomoMode === 'manual'">
                        <p class="text-sm text-slate-800" x-text="pomoManualInput || '自由专注'"></p>
                    </template>
                    <template x-if="pomoMode === 'select'">
                        <ul class="space-y-1">
                            <template x-for="item in pomoQueue">
                                <li class="text-sm text-slate-700 flex items-start gap-1.5">
                                    <span class="text-blue-500 mt-0.5">•</span>
                                    <span x-text="item.title" :class="{'line-through opacity-50': item.completed}"></span>
                                </li>
                            </template>
                        </ul>
                    </template>
                </div>

                <!-- 时间设置 (仅空闲时可见) -->
                <!-- 修复：增加了循环次数 (cycles) 设置 -->
                <div x-show="pomoState.status === 'idle'" class="mt-4 flex items-center justify-between text-xs text-slate-500 border-t border-slate-100 pt-3">
                    <div class="flex gap-2">
                        <label class="flex items-center gap-0.5 cursor-pointer hover:text-blue-600">
                            <span>工作:</span>
                            <input type="number" x-model.number="pomoSettings.workTime" 
                                   @change="logAction('SETTING', `工作时长: ${pomoSettings.workTime}分钟`); save()" 
                                   class="w-6 bg-transparent border-b border-slate-300 focus:border-blue-500 text-center text-slate-700 font-bold outline-none">
                            <span>m</span>
                        </label>
                        <label class="flex items-center gap-0.5 cursor-pointer hover:text-green-600">
                            <span>休息:</span>
                            <input type="number" x-model.number="pomoSettings.breakTime" 
                                   @change="logAction('SETTING', `休息时长: ${pomoSettings.breakTime}分钟`); save()" 
                                   class="w-6 bg-transparent border-b border-slate-300 focus:border-green-500 text-center text-slate-700 font-bold outline-none">
                            <span>m</span>
                        </label>
                        <label class="flex items-center gap-0.5 cursor-pointer hover:text-purple-600 ml-1">
                            <span>循环:</span>
                            <input type="number" x-model.number="pomoSettings.cycles" 
                                   @change="logAction('SETTING', `循环次数: ${pomoSettings.cycles}次`); save()" 
                                   class="w-6 bg-transparent border-b border-slate-300 focus:border-purple-500 text-center text-slate-700 font-bold outline-none">
                            <span>次</span>
                        </label>
                    </div>
                </div>
            </div>
        </div>

        <!-- 任务详情详情面板 (遮盖层) -->
        <div x-show="activeTask" 
             style="display: none;"
             x-transition:enter="transition ease-out duration-200"
             x-transition:enter-start="translate-x-full opacity-50"
             x-transition:enter-end="translate-x-0 opacity-100"
             x-transition:leave="transition ease-in duration-150"
             x-transition:leave-start="translate-x-0 opacity-100"
             x-transition:leave-end="translate-x-full opacity-50"
             class="absolute right-0 top-0 h-full w-[450px] bg-white border-l border-slate-200 shadow-2xl flex flex-col z-30">
            
            <template x-if="activeTask">
                <div class="flex flex-col h-full">
                    <!-- 详情头部 -->
                    <div class="p-5 border-b border-slate-100 flex items-start gap-3 bg-slate-50">
                        <div class="flex-1">
                             <!-- 详情页内的选择开关 -->
                            <div x-show="pomoMode === 'select'" class="mb-2 flex items-center gap-2">
                                <input type="checkbox" :checked="isTaskSelected(activeTask.id)" 
                                       @change="toggleSelectTask(activeTask, columns[activeColIndex].title)"
                                       class="w-4 h-4 text-indigo-500 rounded border-slate-300 focus:ring-indigo-500">
                                <span class="text-xs font-bold text-indigo-600">加入专注列表</span>
                            </div>
                            <!-- 修改：任务标题支持编辑日志 -->
                            <input type="text" x-model="activeTask.title" 
                                   @focus="trackFieldEdit('task-' + activeTask.id, activeTask.title)"
                                   @blur="onFieldBlur('task-' + activeTask.id, activeTask.title, 'task-title')"
                                   class="w-full text-lg font-bold bg-transparent focus:outline-none rounded -ml-1 text-slate-800">
                        </div>
                        <button @click="closeDetail()" class="text-slate-400 hover:text-slate-700 p-1">
                            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path></svg>
                        </button>
                    </div>

                    <!-- 详情内容 -->
                    <div class="flex-1 overflow-y-auto p-5 space-y-6">
                        <!-- 步骤 -->
                        <div>
                            <div class="space-y-2 mb-2">
                                <template x-for="(step, sIndex) in activeTask.steps" :key="step.id">
                                    <div class="flex items-center gap-2 group p-1 hover:bg-slate-50 rounded">
                                        <!-- 步骤选择框 (Focus Mode) -->
                                        <div x-show="pomoMode === 'select'" class="pr-2 border-r border-slate-200 mr-2">
                                            <input type="checkbox" :checked="isStepSelected(step.id)" 
                                                   @change="toggleSelectStep(step, activeTask.title)"
                                                   class="w-3.5 h-3.5 text-indigo-500 rounded border-slate-300">
                                        </div>

                                        <input type="checkbox" x-model="step.completed" 
                                               @change="onStepToggle(step, activeTask.title)" 
                                               class="rounded text-blue-500 w-4 h-4 border-slate-300">
                                        <input type="text" x-model="step.text" 
                                               @focus="trackFieldEdit('step-' + step.id, step.text)"
                                               @blur="onFieldBlur('step-' + step.id, step.text, 'step-text', activeTask.title)"
                                               class="flex-1 text-sm bg-transparent border-none focus:ring-0"
                                               :class="{'line-through text-slate-400': step.completed}">
                                        <button @click="deleteStep(sIndex)" class="text-slate-300 hover:text-red-400 opacity-0 group-hover:opacity-100">×</button>
                                    </div>
                                </template>
                            </div>
                            <!-- 修改：步骤也支持 blur 保存 -->
                            <input type="text" placeholder="+ 下一步骤" @keydown.enter="addStep($event)" @blur="addStep($event)"
                                   class="text-sm w-full py-2 px-3 bg-blue-50/50 rounded text-blue-600 placeholder-blue-300 focus:outline-none">
                        </div>

                        <!-- 备注 -->
                        <div class="space-y-2">
                            <label class="text-xs font-bold text-slate-400 uppercase">备注</label>
                            <textarea x-model="activeTask.content" 
                                      @focus="trackFieldEdit('content-' + activeTask.id, activeTask.content)"
                                      @blur="onFieldBlur('content-' + activeTask.id, activeTask.content, 'task-content', activeTask.title)"
                                      class="w-full bg-slate-50 border border-slate-100 rounded p-3 text-sm text-slate-600 focus:outline-none focus:border-blue-200 h-32 resize-none"></textarea>
                        </div>
                    </div>
                </div>
            </template>
        </div>

    </div>

    <script>
        function todoApp() {
            return {
                // Todo 数据
                columns: [],
                activeColIndex: null,
                activeTaskIndex: null,
                
                // 用于追踪修改前的值
                _editTracking: {},

                // 番茄钟设置与状态
                pomoSettings: {
                    workTime: 25, // 分钟
                    breakTime: 5,
                    cycles: 4
                },
                pomoState: {
                    status: 'idle', // idle, work, break
                    isRunning: false,
                    timeLeft: 25 * 60,
                    totalTime: 25 * 60,
                    cycleCount: 0
                },
                pomoMode: 'manual', // manual, select
                pomoManualInput: '',
                pomoQueue: [], // {id, title, type: 'task'|'step'}
                timerInterval: null,
                
                get activeTask() {
                    if (this.activeColIndex === null || this.activeTaskIndex === null) return null;
                    if (!this.columns[this.activeColIndex] || !this.columns[this.activeColIndex].tasks[this.activeTaskIndex]) return null;
                    return this.columns[this.activeColIndex].tasks[this.activeTaskIndex];
                },

                // 进度条计算
                get timeOffset() {
                    const circumference = 283; // 2 * PI * r(45)
                    const percent = this.pomoState.timeLeft / this.pomoState.totalTime;
                    return circumference - (percent * circumference);
                },

                initApp() {
                    console.log('[前端] 开始初始化');
                    
                    // 等待 pywebview API 准备好
                    const loadData = () => {
                        console.log('[前端] pywebview API 已就绪，调用 get_data()');
                        window.pywebview.api.get_data().then(data => {
                            console.log('[前端] 收到数据:', data);
                            console.log('[前端] 数据类型:', typeof data, '是否数组:', Array.isArray(data));
                            
                            // 兼容新旧数据结构
                            if (Array.isArray(data)) {
                                // 旧数据格式（纯列表），使用默认设置
                                console.log('[前端] 检测到旧数据格式');
                                this.columns = data;
                            } else if (data && data.columns) {
                                // 新数据格式（包含设置）
                                console.log('[前端] 检测到新数据格式，列数:', data.columns.length);
                                this.columns = data.columns;
                                if (data.settings) {
                                    this.pomoSettings = { ...this.pomoSettings, ...data.settings };
                                    console.log('[前端] 加载设置:', this.pomoSettings);
                                }
                            } else {
                                console.error('[前端] 数据格式无法识别:', data);
                            }
                            console.log('[前端] 最终 columns 数量:', this.columns.length);
                            this.$nextTick(() => this.initSortable());
                        }).catch(err => {
                            console.error('[前端] 调用 get_data() 失败:', err);
                        });
                    };
                    
                    // 检查 API 是否已就绪
                    if (window.pywebview && window.pywebview.api) {
                        loadData();
                    } else {
                        console.log('[前端] 等待 pywebview ready 事件...');
                        window.addEventListener('pywebviewready', loadData);
                    }
                },

                // --- 番茄钟逻辑 ---

                formatTime(seconds) {
                    const m = Math.floor(seconds / 60).toString().padStart(2, '0');
                    const s = (seconds % 60).toString().padStart(2, '0');
                    return `${m}:${s}`;
                },

                toggleTimer() {
                    if (this.pomoState.isRunning) {
                        this.pauseTimer();
                    } else {
                        this.startTimer();
                    }
                },

                startTimer() {
                    // 如果是从 Idle 状态启动，初始化时间
                    if (this.pomoState.status === 'idle') {
                        // 准备工作
                        this.pomoState.status = 'work';
                        this.pomoState.totalTime = this.pomoSettings.workTime * 60;
                        this.pomoState.timeLeft = this.pomoState.totalTime;
                        
                        // 构造日志内容
                        let intent = "";
                        if (this.pomoMode === 'manual') {
                            intent = this.pomoManualInput || "未指定";
                        } else {
                            const tasks = this.pomoQueue.map(i => i.title).join(", ");
                            intent = `Tasks: [${tasks}]`;
                        }
                        
                        // 发送日志
                        this.logAction('POMO-START', `时长:${this.pomoSettings.workTime}m | 循环:${this.pomoSettings.cycles} | 内容: ${intent}`);
                    }

                    this.pomoState.isRunning = true;
                    this.timerInterval = setInterval(() => {
                        if (this.pomoState.timeLeft > 0) {
                            this.pomoState.timeLeft--;
                        } else {
                            this.handleTimerComplete();
                        }
                    }, 1000);
                },

                pauseTimer() {
                    this.pomoState.isRunning = false;
                    clearInterval(this.timerInterval);
                    this.logAction('POMO-PAUSE', `剩余: ${this.formatTime(this.pomoState.timeLeft)}`);
                },

                resetTimer() {
                    this.pauseTimer();
                    this.pomoState.status = 'idle';
                    this.pomoState.timeLeft = this.pomoSettings.workTime * 60;
                    this.pomoState.cycleCount = 0;
                    this.logAction('POMO-RESET', '重置计时器');
                },

                handleTimerComplete() {
                    clearInterval(this.timerInterval);
                    this.pomoState.isRunning = false;

                    if (this.pomoState.status === 'work') {
                        this.logAction('POMO-WORK-DONE', `完成 ${this.pomoSettings.workTime}m 专注`);
                        
                        // 切换到休息
                        this.pomoState.cycleCount++;
                        if (this.pomoState.cycleCount >= this.pomoSettings.cycles) {
                            alert("恭喜！你完成了一组完整的番茄钟循环！🎉");
                            this.logAction('POMO-CYCLE-COMPLETE', '完成所有循环');
                            this.resetTimer();
                        } else {
                            this.pomoState.status = 'break';
                            this.pomoState.totalTime = this.pomoSettings.breakTime * 60;
                            this.pomoState.timeLeft = this.pomoState.totalTime;
                            // 自动开始休息倒计时
                            this.startTimer(); 
                        }
                    } else if (this.pomoState.status === 'break') {
                        this.logAction('POMO-BREAK-DONE', '休息结束');
                        // 切换回工作
                        this.pomoState.status = 'work';
                        this.pomoState.totalTime = this.pomoSettings.workTime * 60;
                        this.pomoState.timeLeft = this.pomoState.totalTime;
                        // 休息结束提示
                        alert("休息结束，准备好开始下一个专注时间了吗？");
                        // 可以选择自动开始或等待用户点击，这里保持等待状态
                    }
                },

                // --- 任务选择逻辑 ---

                isTaskSelected(id) {
                    return this.pomoQueue.some(item => item.id === id);
                },

                toggleSelectTask(task, parentTitle) {
                    if (this.isTaskSelected(task.id)) {
                        this.pomoQueue = this.pomoQueue.filter(item => item.id !== task.id);
                        this.logAction('POMO-DESELECT', `移除任务: ${task.title}`);
                    } else {
                        this.pomoQueue.push({
                            id: task.id,
                            title: task.title,
                            type: 'task',
                            parent: parentTitle
                        });
                        this.logAction('POMO-SELECT', `选中任务: ${task.title}`);
                    }
                },

                isStepSelected(id) {
                    return this.pomoQueue.some(item => item.id === id);
                },

                toggleSelectStep(step, taskTitle) {
                    if (this.isStepSelected(step.id)) {
                        this.pomoQueue = this.pomoQueue.filter(item => item.id !== step.id);
                        this.logAction('POMO-DESELECT', `移除步骤: ${taskTitle} - ${step.text}`);
                    } else {
                        this.pomoQueue.push({
                            id: step.id,
                            title: `${taskTitle} - ${step.text}`,
                            type: 'step'
                        });
                        this.logAction('POMO-SELECT', `选中步骤: ${taskTitle} - ${step.text}`);
                    }
                },
                
                removeFromQueue(index) {
                    const item = this.pomoQueue[index];
                    this.pomoQueue.splice(index, 1);
                    this.logAction('POMO-REMOVE', `从队列移除: ${item.title}`);
                },

                // --- 基础 Todo 逻辑 ---
                save() {
                    // 修改：保存数据结构，同时保存任务和设置
                    const dataPayload = {
                        columns: this.columns,
                        settings: this.pomoSettings
                    };
                    const dataStr = JSON.stringify(dataPayload);
                    window.pywebview.api.save_data(dataStr);
                },
                logAction(action, content) {
                    window.pywebview.api.record_activity(action, content);
                },
                
                // 追踪和记录字段修改
                trackFieldEdit(key, currentValue) {
                    if (!this._editTracking[key]) {
                        this._editTracking[key] = currentValue;
                    }
                },
                onFieldBlur(key, newValue, fieldType, parentInfo = '') {
                    const oldValue = this._editTracking[key];
                    if (oldValue !== undefined && oldValue !== newValue && newValue.trim()) {
                        let logMsg = '';
                        if (fieldType === 'column-title') {
                            logMsg = `清单重命名: "${oldValue}" → "${newValue}"`;
                        } else if (fieldType === 'task-title') {
                            logMsg = `任务重命名: "${oldValue}" → "${newValue}"`;
                        } else if (fieldType === 'task-content') {
                            logMsg = `修改备注: ${parentInfo} (${newValue.length}字)`;
                        } else if (fieldType === 'step-text') {
                            logMsg = `修改步骤: ${parentInfo} -> "${oldValue}" → "${newValue}"`;
                        }
                        if (logMsg) {
                            this.logAction('EDIT', logMsg);
                        }
                    }
                    delete this._editTracking[key];
                    this.save();
                },
                onStepToggle(step, taskTitle) {
                    const status = step.completed ? '完成' : '未完成';
                    this.logAction('STEP-TOGGLE', `${taskTitle} -> ${status}: ${step.text}`);
                    this.save();
                },
                toggleColumnCollapse(col) {
                    col.collapsed = !col.collapsed;
                    const state = col.collapsed ? '折叠' : '展开';
                    this.logAction('COLUMN-TOGGLE', `${state}清单: ${col.title}`);
                    this.save();
                },
                addColumn() {
                    const newCol = { id: 'col-' + Date.now(), title: '新清单', collapsed: false, tasks: [] };
                    this.columns.push(newCol);
                    this.logAction('COLUMN-ADD', `创建清单: ${newCol.title}`);
                    this.save();
                    this.$nextTick(() => this.initSortable());
                },
                deleteColumn(index) {
                    if(confirm("删除清单？")) {
                        const colTitle = this.columns[index].title;
                        this.columns.splice(index, 1);
                        this.logAction('COLUMN-DELETE', `删除清单: ${colTitle}`);
                        this.save();
                    }
                },
                addTask(e, colIndex) {
                    const title = e.target.value.trim();
                    if (!title) return; // 如果为空，什么都不做
                    this.columns[colIndex].tasks.push({
                        id: 'task-' + Date.now(), title: title, completed: false, dueDate: '', content: '', steps: []
                    });
                    this.logAction('TODO-ADD', title);
                    e.target.value = ''; // 清空输入框
                    this.save();
                },
                toggleTask(colIndex, taskIndex) {
                    const task = this.columns[colIndex].tasks[taskIndex];
                    task.completed = !task.completed;
                    const status = task.completed ? '完成' : '未完成';
                    this.logAction('TASK-TOGGLE', `${status}: ${task.title}`);
                    this.save();
                },
                openDetail(colIndex, taskIndex) {
                    this.activeColIndex = colIndex;
                    this.activeTaskIndex = taskIndex;
                },
                closeDetail() {
                    this.activeColIndex = null;
                    this.activeTaskIndex = null;
                },
                addStep(e) {
                    const text = e.target.value.trim();
                    if (!text || !this.activeTask) return;
                    this.activeTask.steps.push({ id: 'step-' + Date.now(), text: text, completed: false });
                    this.logAction('STEP-ADD', `${this.activeTask.title} -> 添加步骤: ${text}`);
                    e.target.value = '';
                    this.save();
                },
                deleteStep(index) {
                    const stepText = this.activeTask.steps[index].text;
                    this.activeTask.steps.splice(index, 1);
                    this.logAction('STEP-DELETE', `${this.activeTask.title} -> 删除步骤: ${stepText}`);
                    this.save();
                },
                initSortable() {
                    const colContainer = document.getElementById('columns-container');
                    if (colContainer && !colContainer._sortable) {
                         Sortable.create(colContainer, { handle: '.handle-col', animation: 150, onEnd: (evt) => {
                                const item = this.columns.splice(evt.oldIndex, 1)[0];
                                this.columns.splice(evt.newIndex, 0, item);
                                this.logAction('COLUMN-MOVE', `调整清单顺序: "${item.title}" (${evt.oldIndex + 1}→${evt.newIndex + 1})`);
                                this.save();
                        }});
                        colContainer._sortable = true;
                    }
                    this.columns.forEach((col, index) => {
                        const el = document.getElementById('task-list-' + index);
                        if (el && !el._sortable) {
                            Sortable.create(el, { group: 'tasks', handle: '.handle-task', animation: 150, ghostClass: 'sortable-ghost', onEnd: (evt) => {
                                    const fromColIdx = parseInt(evt.from.id.split('-')[2]);
                                    const toColIdx = parseInt(evt.to.id.split('-')[2]);
                                    const task = this.columns[fromColIdx].tasks.splice(evt.oldIndex, 1)[0];
                                    this.columns[toColIdx].tasks.splice(evt.newIndex, 0, task);
                                    
                                    const fromCol = this.columns[fromColIdx].title;
                                    const toCol = this.columns[toColIdx].title;
                                    if (fromColIdx === toColIdx) {
                                        this.logAction('TASK-REORDER', `"${task.title}" 在 "${fromCol}" 中调整顺序`);
                                    } else {
                                        this.logAction('TASK-MOVE', `"${task.title}": "${fromCol}" → "${toCol}"`);
                                    }
                                    
                                    if(this.activeTask) {
                                        this.closeDetail();
                                    }
                                    this.save();
                            }});
                            el._sortable = true;
                        }
                    });
                }
            }
        }
    </script>
</body>
</html>
"""

if __name__ == '__main__':
    api = Api()
    window = webview.create_window(
        'AI Companion Todo', 
        html=HTML_TEMPLATE,
        js_api=api,
        width=1100,
        height=760,
        min_size=(900, 600)
    )
    webview.start(debug=False)
