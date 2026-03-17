import shutil
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.script_engine.chapter import Chapter
from ling_chat.core.ai_service.type import AdventureConfig, Player, ScriptStatus

from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.game_database.managers.adventure_manager import AdventureManager
from ling_chat.game_database.models import LineAttribute, LineBase, RoleType
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.core.adventure_trigger import adventure_trigger_system

from pathlib import Path

from ling_chat.core.ai_service.exceptions import (
    ChapterLoadError,
    ScriptEngineError,
    ScriptLoadError,
)

SCRIPT_DIR = user_data_path / "game_data" / "scripts"

class ScriptManager:
    def __init__(self, config:AIServiceConfig, game_status:GameStatus):
        # 全局设定，确定剧本状态
        self.config = config
        self.game_status = game_status

        # 全部剧本管理
        self.all_scripts:dict[str, ScriptStatus] = {}
        self._init_all_scripts()

        self.current_script:ScriptStatus|None = None
        self.current_chartper:Chapter|None = None
        self.is_running = False
                 
        if not self.all_scripts:
            logger.error("没有可用的剧本文件")
            return

        # self.init_script()
        self.check_adventure_completion()
    
    def get_script_list(self) -> list[str]:
        return list(self.all_scripts.keys())

    def get_script(self, script_name:str) -> ScriptStatus|None:
        # 添加键值检查
        if script_name not in self.all_scripts:
            logger.error("剧本文件不存在")
            return None
        return self.all_scripts[script_name]

    async def start_script(self, script_name: str) -> bool:
        script = self.get_script(script_name)
        if script is None:
            logger.error("剧本文件不存在")
            return False
        self.current_script = script
        self.is_running = True

        # 初始化剧本
        self._init_script(self.current_script)

        # 导入初始章节，并开始剧本演绎
        await self._run_script(script)

        return True

    def _init_all_scripts(self):
        logger.info("正在" + str(SCRIPT_DIR) + "中寻找剧本")

        if not SCRIPT_DIR.exists() or not SCRIPT_DIR.is_dir():
            logger.warning("剧本文件不存在")
            return

        # 支持分类子目录：character/ 和 standalone/，同时向后兼容平铺结构
        subdirs_to_scan = []
        for category_dir in (SCRIPT_DIR / "character", SCRIPT_DIR / "standalone"):
            if category_dir.exists() and category_dir.is_dir():
                subdirs_to_scan.extend(
                    p for p in category_dir.iterdir() if p.is_dir()
                )
        # 向后兼容：直接放在 scripts/ 根目录下的剧本
        for p in SCRIPT_DIR.iterdir():
            if p.is_dir() and p.name not in ("character", "standalone"):
                subdirs_to_scan.append(p)

        for script_path in subdirs_to_scan:
            config_file = script_path / "story_config.yaml"
            if not config_file.exists():
                continue
            logger.info("找到剧本文件" + script_path.name)
            try:
                script = self._read_script_config(script_path)
                self.all_scripts[script.name] = script
            except Exception as e:
                logger.error(f"读取剧本 {script_path.name} 配置失败: {e}")
    
    def _init_script(self, script: ScriptStatus):
        # 1. 在数据库中，注册所有出场的剧本角色，为游戏状态注册角色，初始化角色设定，台词
        self.game_status.script_status = script
        self._register_script_roles(script)
        # 2. 导入玩家信息
        if script.settings:
            player = Player(script.settings.get("user_name", ""),
                            script.settings.get("user_subtitle", ""),
                            script.settings.get("user_settings", ""))
            # 3. 检查玩家信息是否完整，决定是否导入到GameStatus中
            if player.user_name == "" and player.user_subtitle == "":
                logger.info("本剧本未设定玩家身份，将使用默认玩家身份")
            else:
                self.game_status.player = player

        # 4. 重置游戏状态

    async def _run_script(self, script:ScriptStatus):
        """
        剧本的主执行循环
        """
        if self.current_script is None:
            logger.error("剧本不存在，请先导入剧本")
            return

        self.is_running = True
        script.running_client_id = self.config.last_active_client

        next_chapter_name = self.current_script.intro_chapter

        while next_chapter_name != "end":
            try:
                # 1. 加载章节，返回一个“可运行”的章节对象
                if self._is_adventure_script(script):
                    chapter_path  = SCRIPT_DIR / "character" / script.folder_key / "Chapters" / (next_chapter_name + ".yaml")
                else:
                    chapter_path = SCRIPT_DIR / script.folder_key / "Chapters" / (next_chapter_name + ".yaml")
                current_chapter_obj:Chapter = self._get_chapter(chapter_path, script) # 一个新的辅助方法

                # 2. 命令章节运行，然后等待结果
                next_chapter_name = await current_chapter_obj.run()

            except Exception as e:
                logger.error(f"运行章节 '{next_chapter_name}' 时发生严重错误: {e}", exc_info=True)
                raise ScriptEngineError("运行章节的时候发生错误")

        self.is_running = False

        event_response = ResponseFactory.create_script_end()
        if script.running_client_id:
            await message_broker.publish(script.running_client_id,
                event_response.model_dump()
            )

        logger.info("剧本已经结束。")
        self.game_status.script_status = None

        # 如果是羁绊冒险，标记为已完成
        if self._is_adventure_script(script):
            # 1. 标记运行时完成状态
            self.game_status.completed_scripts.add(script.folder_key)

            # 2. 标记全局完成状态（用于解锁后续冒险）
            from ling_chat.game_database.managers.adventure_manager import AdventureManager
            AdventureManager.mark_global_completed(user_id=1, adventure_folder=script.folder_key)

            # 3. 标记当前存档完成状态
            save_id = self.game_status.active_save_id
            if save_id:
                AdventureManager.mark_completed(save_id, script.folder_key)
            
            # 4. 检查是否有需要解锁的成就和后续剧情
            await self.complete_script(script.folder_key)

            logger.info(f"羁绊冒险 {script.name} 已标记为完成（全局+存档）。")
    
    def _is_adventure_script(self, script: ScriptStatus) -> bool:
        return script.adventure and script.adventure.is_adventure

    def _register_script_roles(self, script: ScriptStatus):
        """从剧本目录读取角色并在数据库中注册"""

        script_key = script.folder_key
        characters_dir = SCRIPT_DIR / script_key / 'characters'

        if not characters_dir.exists() or not characters_dir.is_dir():
            return
            # raise ScriptLoadError(f"剧本 '{script_key}' 中缺少 'characters' 文件夹")

        for character_path in characters_dir.iterdir():
            # 检查是否是目录，并排除特定名称
            if not character_path.is_dir() or character_path.name == 'avatar':
                continue

            settings_path_legacy = character_path / 'settings.txt'
            settings_path = character_path / 'settings.yml'
            if not settings_path.exists() and not settings_path_legacy.exists():
                logger.warning(f"角色目录 '{character_path.name}' 中缺少 settings.txt 或 settings.yml，已跳过。")
                continue

            try:
                settings = Function.load_character_settings(character_path)
                ai_prompt = Function.sys_prompt_builder_by_setting(settings)

                script_role_key = settings.script_role_key
                if script_role_key is None:
                    logger.warning(f"角色目录 '{character_path.name}' 中缺少 script_role_key，已跳过。")
                    continue

                from ling_chat.game_database.managers.role_manager import RoleManager
                role = RoleManager.get_role_by_script_keys(script_key, script_role_key)
                if role is None:
                    role = RoleManager.create_role(title=character_path.name,
                                            type=RoleType.NPC,
                                            resource_folder=character_path.name,
                                            script_key=script_key,
                                            script_role_key=script_role_key
                                            )
                    
                self.game_status.add_line(
                    LineBase(content=ai_prompt,attribute=LineAttribute.SYSTEM,sender_role_id=role.id, display_name=role.name)
                )

            except Exception as e:
                logger.error(f"处理角色 '{character_path.name}' 时出错: {e}", exc_info=True)
                continue

    def _read_script_config(self, script_path):
        config = Function.read_yaml_file( script_path / "story_config.yaml" )
        if config is not None:
            adventure = AdventureConfig.from_dict(config.get('adventure'))
            return ScriptStatus(folder_key=script_path.name,
                                name=config.get('script_name', 'ERROR'),
                                description=config.get('description', 'ERROR'),
                                intro_chapter=config.get('intro_chapter', 'ERROR'), 
                                settings=config.get('script_settings', {}),
                                adventure=adventure,
                                )
        else:
            raise ScriptLoadError("剧本读取出现错误,缺少 story_config.yml 配置文件")

    def get_character_adventures(self, character_folder: str) -> list[ScriptStatus]:
        """获取指定角色绑定的所有羁绊冒险，按 order 排序"""
        adventures = [
            s for s in self.all_scripts.values()
            if s.adventure.is_adventure and s.adventure.bound_character_folder == character_folder
        ]
        adventures.sort(key=lambda s: s.adventure.order)
        return adventures

    def get_all_adventures(self) -> list[ScriptStatus]:
        """获取所有已注册的羁绊冒险"""
        return [
            s for s in self.all_scripts.values()
            if s.adventure.is_adventure
        ]
    
    def _get_chapter(self, chapter_path: Path, script_status: ScriptStatus) -> Chapter:
        config = Function.read_yaml_file(chapter_path)
        if config is not None:
            return Chapter(str(chapter_path), self.config, self.game_status, config, script_status)
        else:
            raise ChapterLoadError(f"导入 {chapter_path} 剧本的时候出现问题")
    
    async def complete_script(self, script_folder: str) -> None:
        user_id = 1
        AdventureManager.mark_global_completed(user_id, script_folder)

        unlocked_achievements = []
        for name, s in self.all_scripts.items():
            if s.folder_key == script_folder and s.adventure.completion_achievements:
                from ling_chat.core.achievement_manager import achievement_manager
                for ach_def in s.adventure.completion_achievements:
                    ach_id = ach_def.get("id")
                    if ach_id:
                        result = achievement_manager.unlock(ach_id, ach_def)
                        if result:
                            unlocked_achievements.append(result)
                break

        # 检查是否有后续冒险被解锁
        # 注意：这里需要user_id，但我们没有从请求中获取
        # 暂时使用默认值1，后续可以改进
        self.check_adventure_completion()

    def check_adventure_completion(self) -> None:
        user_id = 1  # TODO: 从session或其他地方获取真实的user_id
        all_adventures = self.get_all_adventures()
        chat_count = self.game_status.get_chat_message_count()
        adventure_trigger_system.check_all_adventures(
            user_id=user_id,
            adventures=all_adventures,
            chat_count=chat_count,
        )
    
    def get_assets_dir(self, script_name: str | None = None) -> Path:
        """
        获取剧本 Assets 目录。
        - 若 script_name=None，则使用当前正在运行的剧本；若尚未开始剧本，则尝试使用列表第一个剧本。
        """
        script: ScriptStatus | None = None
        if script_name:
            script = self.get_script(script_name)
        else:
            script = self.current_script
            if script is None:
                names = self.get_script_list()
                if names:
                    script = self.get_script(names[0])

        if script is None:
            raise ScriptLoadError("没有可用的剧本，无法定位资源目录")

        return SCRIPT_DIR / script.folder_key / "Assets"

    # 兼容旧拼写（assests）
    def get_assests_dir(self) -> Path:
        return self.get_assets_dir()

    def get_avatar_dir(self, character: str, script_name: str | None = None) -> Path:
        """
        获取指定角色的 avatar 目录。
        character 通常为剧本里的 script_role_key（也可能就是角色文件夹名）。
        """
        script: ScriptStatus | None = None
        if script_name:
            script = self.get_script(script_name)
        else:
            script = self.current_script
            if script is None:
                names = self.get_script_list()
                if names:
                    script = self.get_script(names[0])

        if script is None:
            raise ScriptLoadError("没有可用的剧本，无法定位角色资源目录")

        char_dir = SCRIPT_DIR / script.folder_key / "characters" / character
        # 兼容大小写/不同命名
        for candidate in ("avatar", "Avatar", "avatars", "Avatars"):
            p = char_dir / candidate
            if p.exists() and p.is_dir():
                return p

        return char_dir

    def get_script_characters(self, script_name: str) -> list[dict]:
        """
        读取剧本的角色设置，返回给前端用于初始化 UI。
        返回的 dict 尽量包含：character_id(角色ID/roleId), ai_name/ai_subtitle, scale/offset/bubble 等。
        """
        script = self.get_script(script_name)
        if script is None:
            raise ScriptLoadError("剧本文件不存在")

        characters_dir = SCRIPT_DIR / script.folder_key / "characters"
        if not characters_dir.exists() or not characters_dir.is_dir():
            return []
            # raise ScriptLoadError(f"剧本 '{script.folder_key}' 中缺少 'characters' 文件夹")
        

        results: list[dict] = []

        from ling_chat.game_database.managers.role_manager import RoleManager

        for character_path in characters_dir.iterdir():
            if not character_path.is_dir():
                continue

            if character_path.name.lower() == "avatar":
                continue

            settings_path_legacy = character_path / "settings.txt"
            settings_path = character_path / "settings.yml"
            if not settings_path.exists() and not settings_path_legacy.exists():
                logger.warning(f"角色目录 '{character_path.name}' 中缺少 settings.txt 或 settings.yml，已跳过。")
                continue

            settings = Function.load_character_settings(character_path) or {}
            script_role_key = settings.script_role_key
            if not script_role_key:
                logger.warning(f"角色目录 '{character_path.name}' 中缺少 script_role_key，已跳过。")
                continue
            
            role = RoleManager.get_role_by_script_keys(script.folder_key, script_role_key)

            settings_out = dict(settings)
            settings_out["character_id"] = getattr(role, "id", -1) if role else -1
            settings_out.setdefault("ai_name", settings.ai_name or character_path.name)
            settings_out.setdefault("ai_subtitle", settings.ai_subtitle or "")

            results.append(settings_out)

        return results