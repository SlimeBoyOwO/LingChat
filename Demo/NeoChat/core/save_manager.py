import os
import shutil
import yaml
from datetime import datetime
from typing import Dict, Optional

import config
from .models import (GameSession, GameState, GameProgress, ProgressPointer, 
                     Character, Player, StoryPack, StoryUnit)
from .logger import log_info, log_error, log_debug

class SaveManager:
    """处理所有与文件系统相关的加载和保存操作。"""
    def _load_yaml(self, path: str) -> Optional[Dict]:
        try:
            with open(path, 'r', encoding='utf-8') as f:
                return yaml.safe_load(f)
        except FileNotFoundError:
            log_error(f"文件未找到: {path}")
            return None
        except yaml.YAMLError as e:
            log_error(f"解析YAML文件失败: {path}, 错误: {e}")
            return None

    def _save_yaml(self, path: str, data: Dict):
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, 'w', encoding='utf-8') as f:
                yaml.dump(data, f, allow_unicode=True, sort_keys=False)
        except Exception as e:
            log_error(f"保存YAML文件失败: {path}, 错误: {e}")
            
    def load_story_pack_config(self, pack_path: str) -> Optional[StoryPack]:
        config_path = os.path.join(pack_path, 'story_config.yaml')
        data = self._load_yaml(config_path)
        if data:
            return StoryPack(
                pack_id=data.get('id', 'unknown'),
                start_unit_id=data.get('start_unit_id'),
                character_roles=data.get('character_roles', []),
                dm_role_id=data.get('dm_role_id')
            )
        return None

    def load_story_units(self, pack_path: str) -> Dict[str, StoryUnit]:
        story_dir = os.path.join(pack_path, 'story')
        units = {}
        if not os.path.isdir(story_dir):
            return units
        for filename in os.listdir(story_dir):
            if filename.endswith('.yaml'):
                unit_id = filename.split('.')[0]
                unit_path = os.path.join(story_dir, filename)
                data = self._load_yaml(unit_path)
                if data:
                    units[unit_id] = StoryUnit(
                        unit_id=unit_id,
                        events=data.get('Events', []),
                        end_condition=data.get('EndCondition')
                    )
        return units

    def create_new_game_session(self, story_pack_path: str, character_selections: Dict[str, str], player_data: Dict) -> Optional[GameSession]:
        """创建一个全新的游戏会话，并生成存档目录。"""
        try:
            # 1. 创建存档文件夹
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            save_path = os.path.join(config.SAVES_BASE_PATH, f"save_{timestamp}")
            os.makedirs(os.path.join(save_path, "save"), exist_ok=True)
            # 2. 复制剧本和角色文件
            shutil.copytree(story_pack_path, save_path, dirs_exist_ok=True)
            char_dir_in_save = os.path.join(save_path, "character")
            os.makedirs(char_dir_in_save, exist_ok=True)
            
            characters = {}
            for role_id, char_path in character_selections.items():
                dest_path = os.path.join(char_dir_in_save, f"{os.path.basename(char_path)}")
                shutil.copy(char_path, dest_path)
                char_data = self._load_yaml(dest_path)
                if char_data:
                    characters[role_id] = Character(role_id=role_id, name=char_data.get('name'), prompt=char_data.get('prompt'))

            # 3. 加载配置和剧情
            story_pack_config = self.load_story_pack_config(save_path)
            if not story_pack_config: return None
            story_units = self.load_story_units(save_path)

            # 4. 初始化状态
            initial_gamestate_data = self._load_yaml(os.path.join(save_path, 'save', 'gamestate.yaml')) or {}
            game_state = GameState(variables=initial_gamestate_data)
            
            player = Player(name=player_data.get('player_name', '玩家'), prompt=player_data.get('player_prompt', ''))
            game_state.set('player_name', player.name) # 将玩家名注入初始状态
            for role_id, char in characters.items():
                 game_state.set(f'character_name_{role_id}', char.name)

            game_progress = GameProgress(
                save_name="New Game",
                story_pack_id=story_pack_config.pack_id,
                last_saved_timestamp=datetime.now().isoformat(),
                pointer=ProgressPointer(current_unit_id=story_pack_config.start_unit_id)
            )

            session = GameSession(
                save_path=save_path,
                game_state=game_state,
                game_progress=game_progress,
                dialogue_history=[],
                story_pack_config=story_pack_config,
                story_units=story_units,
                characters=characters,
                player=player
            )

            # 5. 执行一次初始保存
            self.save_game_session(session, "初始存档")
            log_info(f"新游戏已创建, 存档位于: {save_path}")
            return session

        except Exception as e:
            log_error(f"创建新游戏失败: {e}", exc_info=True)
            return None

    def save_game_session(self, session: GameSession, save_name: Optional[str] = None):
        """将整个游戏会话保存到文件。"""
        if save_name:
            session.game_progress.save_name = save_name
        session.game_progress.last_saved_timestamp = datetime.now().isoformat()
        
        save_dir = os.path.join(session.save_path, 'save')
        
        self._save_yaml(os.path.join(save_dir, 'gamestate.yaml'), session.game_state.variables)
        self._save_yaml(os.path.join(save_dir, 'dialogue_history.yaml'), session.dialogue_history)
        
        progress_dict = {
            "save_name": session.game_progress.save_name,
            "story_pack_id": session.game_progress.story_pack_id,
            "last_saved_timestamp": session.game_progress.last_saved_timestamp,
            "runtime_state": session.game_progress.runtime_state,
            "context": session.game_progress.context,
            "progress_pointer": {
                "current_unit_id": session.game_progress.pointer.current_unit_id,
                "last_completed_event_index": session.game_progress.pointer.last_completed_event_index
            }
        }
        self._save_yaml(os.path.join(save_dir, 'game_progress.yaml'), progress_dict)

        log_info(f"游戏已保存, 存档名: '{session.game_progress.save_name}'")


    def load_game_session(self, save_path: str) -> Optional[GameSession]:
        """从存档目录加载完整的游戏会话。"""
        try:
            save_dir = os.path.join(save_path, 'save')

            # 加载核心数据
            gamestate_data = self._load_yaml(os.path.join(save_dir, 'gamestate.yaml')) or {}
            progress_data = self._load_yaml(os.path.join(save_dir, 'game_progress.yaml'))
            dialogue_data = self._load_yaml(os.path.join(save_dir, 'dialogue_history.yaml')) or []

            if not progress_data:
                log_error("存档损坏: 缺少 game_progress.yaml")
                return None
            
            # 加载剧本和角色
            story_pack_config = self.load_story_pack_config(save_path)
            story_units = self.load_story_units(save_path)

            char_dir = os.path.join(save_path, "character")
            characters = {}
            if os.path.isdir(char_dir):
                for filename in os.listdir(char_dir):
                    if filename.endswith('.yaml'):
                        role_id = os.path.splitext(os.path.basename(filename))[0]
                        char_data = self._load_yaml(os.path.join(char_dir, filename))
                        if char_data:
                            characters[role_id] = Character(role_id=role_id, name=char_data.get('name'), prompt=char_data.get('prompt'))

            game_state = GameState(variables=gamestate_data)
            pointer_data = progress_data.get('progress_pointer', {})
            game_progress = GameProgress(
                save_name=progress_data.get('save_name', 'Loaded Save'),
                story_pack_id=progress_data.get('story_pack_id'),
                last_saved_timestamp=progress_data.get('last_saved_timestamp'),
                runtime_state=progress_data.get('runtime_state', 'ExecutingEvents'),
                context=progress_data.get('context', {}),
                pointer=ProgressPointer(
                    current_unit_id=pointer_data.get('current_unit_id'),
                    last_completed_event_index=pointer_data.get('last_completed_event_index', -1)
                )
            )

            player = Player(name=game_state.get('player_name', '玩家'))

            log_info(f"成功从 {save_path} 加载存档。")
            return GameSession(
                save_path=save_path,
                game_state=game_state,
                game_progress=game_progress,
                dialogue_history=dialogue_data,
                story_pack_config=story_pack_config,
                story_units=story_units,
                characters=characters,
                player=player
            )
            
        except Exception as e:
            log_error(f"加载存档失败: {e}", exc_info=True)
            return None