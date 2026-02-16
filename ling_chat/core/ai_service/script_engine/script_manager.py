import shutil
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.script_engine.charpter import Charpter
from ling_chat.core.ai_service.type import Player, ScriptStatus

from ling_chat.core.logger import logger
from ling_chat.game_database.models import LineAttribute, LineBase, RoleType
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path

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
        self.current_chartper:Charpter|None = None
        self.is_running = False
                 
        if not self.all_scripts:
            logger.error("没有可用的剧本文件")
            return

        # self.init_script()
    
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

        for script_path in SCRIPT_DIR.iterdir():
            if not script_path.is_dir():  # 排除非目录
                continue
            logger.info("找到剧本文件" + script_path.name)
            script = self._read_script_config(script_path)
            self.all_scripts[script.name] = script
    
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

        next_charpter_name = self.current_script.intro_charpter

        while next_charpter_name != "end":
            try:
                # 1. 加载章节，返回一个“可运行”的章节对象
                charpter_path = SCRIPT_DIR / script.folder_key / "Charpters" / (next_charpter_name + ".yaml")
                current_charpter_obj:Charpter = self._get_charpter(charpter_path) # 一个新的辅助方法

                # 2. 命令章节运行，然后等待结果
                next_charpter_name = await current_charpter_obj.run()

            except Exception as e:
                logger.error(f"运行章节 '{next_charpter_name}' 时发生严重错误: {e}", exc_info=True)
                raise ScriptEngineError("运行章节的时候发生错误")

        self.is_running = False
        logger.info("剧本已经结束。")

    def _register_script_roles(self, script: ScriptStatus):
        """从剧本目录读取角色并在数据库中注册"""

        script_key = script.folder_key
        characters_dir = SCRIPT_DIR / script_key / 'characters'

        if not characters_dir.exists() or not characters_dir.is_dir():
            raise ScriptLoadError(f"剧本 '{script_key}' 中缺少 'characters' 文件夹")

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
            return ScriptStatus(folder_key=script_path.name,
                                name=config.get('script_name', 'ERROR'),
                                description=config.get('description', 'ERROR'),
                                intro_charpter=config.get('intro_charpter', 'ERROR'), 
                                settings=config.get('script_settings', {})
                                )
        else:
            raise ScriptLoadError("剧本读取出现错误,缺少 story_config.yml 配置文件")
    
    def _get_charpter(self, charpter_path: Path) -> Charpter:
        config = Function.read_yaml_file(charpter_path)
        if config is not None:
            return Charpter(str(charpter_path), self.config, self.game_status, config.get('events',[]), config.get('end',{}))
        else:
            raise ChapterLoadError(f"导入 {charpter_path} 剧本的时候出现问题")
    
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
            raise ScriptLoadError(f"剧本 '{script.folder_key}' 中缺少 'characters' 文件夹")

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