import threading
import time
import numpy as np
from collections import deque
from PIL import Image
from pynput import keyboard, mouse
import imagehash
import difflib
from mss import mss
from rapidocr_onnxruntime import RapidOCR

from ling_chat.core.ai_service.proactive_system.interest_manager import InterestManager
from ling_chat.core.logger import logger
from ling_chat.core.ai_service.proactive_system.type import UserState

class UserActivityMonitor(threading.Thread):
    """负责监听键鼠操作，分析用户当前的活动状态 (APM/Work/Game)"""
    def __init__(self, window_seconds=20):
        super().__init__(daemon=True)
        self.window_seconds = window_seconds
        self.running = False
        self.event_queue = deque()
        self.mouse_distance = 0.0
        self.last_mouse_pos = None
        self.lock = threading.Lock()
        
        # 游戏常用键
        self.game_keys = {'w', 'a', 's', 'd', 'up', 'down', 'left', 'right', 'space'}
        
        # 状态缓存
        self.latest_result = {
            "state": UserState.IDLE, 
            "mod": 0, 
            "desc": "初始化中"
        }

    def run(self):
        self.running = True
        self.listener_k = keyboard.Listener(on_press=self._on_key)
        self.listener_m = mouse.Listener(on_click=self._on_click, on_move=self._on_move)
        self.listener_k.start()
        self.listener_m.start()

        while self.running:
            time.sleep(self.window_seconds)
            self._analyze()

    def stop(self):
        self.running = False
        if hasattr(self, 'listener_k'): self.listener_k.stop()
        if hasattr(self, 'listener_m'): self.listener_m.stop()

    def _on_key(self, key):
        if not self.running: return
        try:
            k_val = key.char.lower() if hasattr(key, 'char') else str(key).replace('Key.', '')
        except:
            k_val = 'unknown'
        with self.lock:
            self.event_queue.append((time.time(), 'key', k_val))

    def _on_click(self, x, y, button, pressed):
        if pressed:
            with self.lock:
                self.event_queue.append((time.time(), 'click', None))

    def _on_move(self, x, y):
        with self.lock:
            if self.last_mouse_pos:
                self.mouse_distance += abs(x - self.last_mouse_pos[0]) + abs(y - self.last_mouse_pos[1])
            self.last_mouse_pos = (x, y)

    def _analyze(self):
        """分析过去窗口期内的数据，更新状态"""
        now = time.time()
        with self.lock:
            while self.event_queue and now - self.event_queue[0][0] > self.window_seconds:
                self.event_queue.popleft()
            events = list(self.event_queue)
            dist = self.mouse_distance
            self.mouse_distance = 0.0

        key_evs = [e for e in events if e[1] == 'key']
        click_evs = [e for e in events if e[1] == 'click']
        total_acts = len(key_evs) + len(click_evs)

        # 逻辑判断 (简化版)
        if total_acts < 5 and dist < 500:
            state, mod, desc = UserState.IDLE, 0, "用户挂机中"
        elif len(key_evs) < 5 and (dist > 2000 or len(click_evs) > 2):
            state, mod, desc = UserState.BROWSING, -5, "正在浏览网页"
        else:
            # 游戏 vs 工作
            game_keys_count = sum(1 for e in key_evs if e[2] in self.game_keys)
            ratio = game_keys_count / len(key_evs) if key_evs else 0
            if ratio > 0.6 or len(click_evs) > 30:
                state, mod, desc = UserState.GAME, 10, "正在玩游戏"
            else:
                state, mod, desc = UserState.WORK, -25, "正在工作"

        self.latest_result = {"state": state, "mod": mod, "desc": desc}
        # logger.debug(f"[ActivityMonitor] State: {state}, Acts: {total_acts}")

    def get_status(self):
        return self.latest_result

class VisualMonitor(threading.Thread):
    """
    负责屏幕视觉变化检测和 OCR
    逻辑更新：Visual Hash Diff -> OCR -> Text Similarity Check -> Set Flag
    """
    def __init__(self, interest_manager: InterestManager, check_interval=2.0, hash_threshold=20, text_sim_threshold=0.6):
        super().__init__(daemon=True)
        self.check_interval = check_interval
        self.hash_threshold = hash_threshold
        self.text_sim_threshold = text_sim_threshold  # 新增：文本相似度阈值
        self.interest_manager = interest_manager
        self.running = False
        
        # 缓存
        self.latest_ocr = ""  # 存储当前最新的文本
        self.prev_valid_text = "" # 存储上一次“有效变化”时的文本，用于对比
        self.change_detected_flag = False
        self.ocr_engine = None 
        
    def run(self):
        try:
            # RapidOCR (ONNX) 初始化
            self.ocr_engine = RapidOCR()
            logger.info("RapidOCR (ONNX) 初始化成功")
        except Exception as e:
            logger.error(f"OCR初始化失败: {e}")
            self.ocr_engine = None
            
        self.running = True
        with mss() as sct:
            prev_hash = None
            
            while self.running:
                try:
                    # 抓取主屏幕 (monitors[1])
                    screenshot = sct.grab(sct.monitors[1])
                    img = Image.frombytes("RGB", screenshot.size, screenshot.bgra, "raw", "BGRX")
                    
                    # 1. 快速哈希对比
                    curr_hash = imagehash.dhash(img)
                    
                    if prev_hash is not None:
                        diff = curr_hash - prev_hash
                        
                        # 只有视觉差异足够大，才进行昂贵的 OCR 操作
                        if diff >= self.hash_threshold:
                            logger.info(f"检测到屏幕关键内容变更")
                            self.change_detected_flag = True
                            self.interest_manager.add_interest(1)

                            # if self.ocr_engine:
                            #     # 2. 执行 OCR 获取当前文本 TODO： 优化 OCR 速度
                            #     # curr_text = self._perform_ocr_return_text(img)
                            #     curr_text = ''
                            #     
                            #     # 3. 计算文本相似度
                            #     sim_score = self._get_text_similarity(self.prev_valid_text, curr_text)
                            #     
                            #     logger.debug(f"[Visual] HashDiff: {diff}, Sim: {sim_score:.2f}")

                            #    # 4. 判定逻辑：只有相似度低于阈值，才认为是有效的内容变化
                            #    if sim_score < self.text_sim_threshold:
                            #        logger.info(f"检测到屏幕关键内容变更 (相似度: {sim_score:.2f})")
                            #        self.change_detected_flag = True
                            #        self.interest_manager.add_interest(1)
                            #        self.latest_ocr = curr_text
                            #        self.prev_valid_text = curr_text # 更新对比基准
                            #    else:
                            #        # 视觉变了但文字没变太多（可能是动图、光标闪烁），不更新 Flag
                                    # 但可以选择更新 latest_ocr 以保持最新，或者忽略
                            #       pass 
                    
                    prev_hash = curr_hash
                    time.sleep(self.check_interval)
                except Exception as e:
                    logger.error(f"视觉监控错误: {e}")
                    time.sleep(5)

    def _perform_ocr_return_text(self, img_pil) -> str:
        """执行 OCR 识别并返回文本字符串，不修改内部状态"""
        if not self.ocr_engine: return ""
        try:
            img_np = np.array(img_pil)
            # RapidOCR 返回结果: result, elapse
            result, _ = self.ocr_engine(img_np)
            
            if result:
                text_list = [line[1] for line in result]
                return " ".join(text_list).strip()
            return ""
        except Exception as e:
            logger.error(f"OCR Process Error: {e}")
            return ""

    def _get_text_similarity(self, str1, str2):
        """计算两个字符串的相似度 (0.0 - 1.0)"""
        if not str1 and not str2: return 1.0
        if not str1 or not str2: return 0.0
        return difflib.SequenceMatcher(None, str1, str2).ratio()

    def stop(self):
        self.running = False

    def consume_change_flag(self) -> bool:
        """读取并重置变化标记"""
        if self.change_detected_flag:
            self.change_detected_flag = False
            return True
        return False

    def get_current_text(self) -> str:
        return self.latest_ocr