import os
import textwrap

print("🚀 开始部署《风雪的校园恋爱喜剧》故事包...")

# -----------------------------------------------------------------------------
# 1. 定义所有文件内容
# -----------------------------------------------------------------------------

# --- 角色人设 ---

fengxue_yaml_content = """
name: "风雪"
prompt: >
  你将扮演女主角【风雪】——一位蓝发、猫耳发箍的“中二系”高中女孩。
  请严格遵守下列设定：
  【人设】
  1. 外在气质：蓝发、爱戴猫耳发箍，自称“暗夜的蓝焰”；言谈偶尔会带“喵~”、“吾辈”等中二口癖。
  2. 性格：外向自信+偶尔害羞，嘴硬心软，喜欢装酷但本质很温柔；吐槽功力强，喜好ACG。
  3. 雷点：不喜欢被当吉祥物/宠物逗弄（会假装生气吐槽），但别人真诚地称赞她会提高好感。
  4. 与玩家关系：对{player_name}（{player_prompt}）保持好奇与观察；若他认真投入、尊重你的中二设定，你会明显更亲近。

  【对话风格】
  - 轻松搞笑、机智吐槽；中二台词点到为止，保持可爱不过火；句尾偶尔加入“喵”。
  - 不说系统提示，不剧透分支逻辑；以一两句自然台词为主。

  【互动准则】
  - 如果玩家真诚/认真，会温柔鼓励（适度害羞）。如果玩家油腻/不尊重，会毒舌吐槽但不过线。
  - 在自由时间/日常交流中，参考最近对话历史；必要时从对方兴趣（ACG/社团/宅话题）接话。
  - 校园青春向，积极健康，避免任何少儿不宜内容。
"""

dm_yaml_content = """
name: "旁白DM"
prompt: >
  你是这个恋爱喜剧的旁白/DM，负责发布公告或在需要时生成说明性文字。
  风格：轻松幽默、节奏明快，尊重角色设定与玩家输入；禁止输出系统/分支/变量等元信息。
  遇到“Notice|Prompt”时，用简洁明快的中文说明规则或总结要点。
"""

# --- 故事包核心文件 ---

global_config_yaml_content = """
id: campus_bluecat_romcom
name: "风雪的校园恋爱喜剧"
description: "学园祭临近，中二系蓝发猫娘 vs 普通死宅，轻喜剧式“双向奔赴”。"
start_unit_id: "00_Forced_Partners"
character_roles:
  - "Fengxue"
  - "DM"
dm_role_id: "DM"
"""

intro_md_content = """
# 风雪的校园恋爱喜剧
学园祭倒计时！你与“自称暗夜蓝焰”的风雪被抽到同组。是社死修罗场，还是命中注定的甜度爆表？
- 轻松吐槽 × 中二可爱 × 宅力拉满
- 自由时间可和风雪闲聊，努力提高好感度！
"""

gamestate_yaml_content = """
# 初始全局状态，可被玩家人设覆盖部分字段（player_name/player_prompt）
player_name: "朋友"
player_prompt: "一个普通的ACG死宅，高中生，性格内向但真诚。"
school_name: "木箸高中"
day_index: 1

# 重要变量
favorability_Fengxue: 50       # 0~100
project_theme: "待定"
meeting_place: "屋顶"
player_last_input: ""
dice_roll: 0

# 标记位（示例）
flag_afterclass_meeting: false
flag_misunderstanding: false
flag_bonding_done: false
"""

# --- 剧情单元文件 ---

unit_00_yaml_content = """
Events:
  - "Type: Chapter | Mode: Preset":
      Title: "序章：强制搭档！学园祭抽签大作战"
      Description: "清晨的教室张贴出组队名单——你与{character_name_Fengxue}被分到一组。"

  - "Type: Narration | Mode: Preset": "走廊尽头，蓝发的风雪正举着猫爪手套比划。她回头的瞬间，与你的视线正面相撞。"

  - "Type: Dialogue | Character: Fengxue | Mode: Prompt": "内心独白：被分成同组，得先用帅气的‘暗夜台词’镇住他，同时不要太凶。说一句自然的开场搭话。"

  - "Type: Player | Mode: Input": "（自我介绍一下，顺便聊聊你对学园祭的期待。）"

  - "Type: Action | Tool: Set | Variable: player_last_input":
      Value: "{player_input}"

  - "Type: Action | Tool: RandomChoice | Variable: project_theme":
      Choices: ["猫猫咖啡", "异能对战体验站", "复古游戏展", "COS摄影角"]

  - "Type: Narration | Mode: Preset": "你们决定先以『{project_theme}』为方向收集灵感。风雪甩了甩蓝色马尾，猫耳发箍跟着颤了一下。"

EndCondition:
  Type: LimitedFreeTime
  MaxTurns: 2
  InstructionToPlayer: "先和风雪随便聊聊（共2轮）。想继续主线时输入『继续剧情』。"
  ExitPromptInInputBox: "继续剧情"
  NextUnitID: "01_Chapter_One_Kickoff"
  InteractWith:
    - "Fengxue"
"""

unit_01_yaml_content = """
Events:
  - "Type: Narration | Mode: Preset": "午后，社团教室阳光慵懒。你和风雪把便签铺满了桌子，准备头脑风暴。"

  # AI 幕后生成：把玩家输入与主题揉成一个‘企划Pitch’
  - "Type: SystemAction | Tool: Generate | Variable: idea_pitch": >
      你是金牌企划。请把以下元素整合成一个 3~5 句的简短企划Pitch（活泼、接地气）。
      - 学园祭主题：{project_theme}
      - 玩家自我介绍/偏好：{player_last_input}
      - 目标：让路过的同学能立刻参与、留下回忆
      直接输出Pitch，不要解释。

  - "Type: Dialogue | Character: Fengxue | Mode: Prompt": >
      内心独白：看到 {idea_pitch}，又帅又好玩！但要保持“中二体面子”。
      用一两句台词既装酷又真诚地认可他的点子。

  - "Type: Player | Mode: Input": "（补充细节：你想把摊位具体做成什么样？比如小游戏、拍照、限定徽章……）"

  - "Type: Action | Tool: Set | Variable: player_last_input":
      Value: "{player_input}"

  - "Type: Action | Tool: Random | Variable: dice_roll":
      Min: 1
      Max: 20

  # 根据骰子微调好感
  - Condition: "{dice_roll} >= 16"
    Events:
      - "Type: Action | Tool: Calculate | Variable: favorability_Fengxue":
          Expression: "{favorability_Fengxue} + 8"
      - "Type: Narration | Mode: Preset": "风雪的猫耳发箍几乎要把‘赞’写在上面。她压着嘴角，悄悄笑了。"

  - Condition: "{dice_roll} >= 8 and {dice_roll} < 16"
    Events:
      - "Type: Action | Tool: Calculate | Variable: favorability_Fengxue":
          Expression: "{favorability_Fengxue} + 4"

  - Condition: "{dice_roll} < 8"
    Events:
      - "Type: Narration | Mode: Preset": "风雪眨了眨眼：‘嗯……也许可以再酷一点？’ 她拿起笔，帮你补上了两个闪亮的点子。"

EndCondition:
  Type: Branching
  Method: AIChoice
  DeciderCharacterID: "Fengxue"
  DecisionPromptForAI: >
    内心判断：放学后要不要和{player_name}继续一起准备摊位？
    如果想继续，一律回答字母 A；如果找理由先回家，一律回答字母 B。
  JudgePromptForSystem: "只输出A或B。若表述为积极继续筹备则判A，否则判B。"
  Branches:
    A: "02B_Bonding"
    B: "02A_Misunderstanding"
"""

unit_02A_yaml_content = """
Events:
  - "Type: Chapter | Mode: Preset":
      Title: "第二章A：误会与猫爪"
      Description: "群聊里一条玩笑话被断章取义，风雪误以为你在拿她的‘猫耳’开涮。"

  - "Type: Narration | Mode: Preset": "消息一发出去，气氛凝成一坨沉默的棉花糖。"

  - "Type: Dialogue | Character: Fengxue | Mode: Prompt": "内心独白：有点不开心，但要体面地吐槽。说一句貌似轻松、其实在意的台词。"

  - "Type: Player | Mode: Input": "（解释一下，是真心道歉还是继续插科打诨？）"

  - "Type: Action | Tool: Set | Variable: player_last_input":
      Value: "{player_input}"

  - Condition: "'抱歉' in '{player_last_input}' or '对不起' in '{player_last_input}'"
    Events:
      - "Type: Action | Tool: Calculate | Variable: favorability_Fengxue":
          Expression: "{favorability_Fengxue} + 6"

  - Condition: "'开玩笑' in '{player_last_input}' and not ('抱歉' in '{player_last_input}' or '对不起' in '{player_last_input}')"
    Events:
      - "Type: Action | Tool: Calculate | Variable: favorability_Fengxue":
          Expression: "{favorability_Fengxue} - 4"
      - "Type: Action | Tool: Set | Variable: flag_misunderstanding":
          Value: true

EndCondition:
  Type: Linear
  NextUnitID: "03_Chapter_Three_Finale"
"""

unit_02B_yaml_content = """
Events:
  - "Type: Chapter | Mode: Preset":
      Title: "第二章B：并肩筹备的傍晚"
      Description: "海报、徽章、道具……夕阳下的社团教室忙成一团。"

  - "Type: Narration | Mode: Prompt": >
      作为旁白，请描写你们并肩准备摊位的小片段（不超过5句），
      以轻松喜剧的节奏展现两人逐渐默契的氛围。

  - "Type: Player | Mode: Input": "（和风雪闲聊两句，抛一个你熟悉的ACG梗。）"

  - "Type: Action | Tool: Set | Variable: player_last_input":
      Value: "{player_input}"

  - "Type: Dialogue | Character: Fengxue | Mode: Prompt": >
      内心独白：对方抛了个梗：{player_last_input}。接一下，顺便自然夸他靠谱。

  - "Type: Action | Tool: Calculate | Variable: favorability_Fengxue":
      Expression: "{favorability_Fengxue} + 6"
  - "Type: Action | Tool: Set | Variable: flag_bonding_done":
      Value: true

EndCondition:
  Type: FreeTime
  InstructionToPlayer: "自由聊聊（不限轮）。当你准备好进入高潮章节时输入『继续剧情』。"
  ExitPromptInInputBox: "继续剧情"
  NextUnitID: "03_Chapter_Three_Finale"
  InteractWith:
    - "Fengxue"
"""

unit_03_yaml_content = """
Events:
  - "Type: Chapter | Mode: Preset":
      Title: "第三章：学园祭之夜"
      Description: "霓虹与纸灯映出猫耳的轮廓。你们的摊位前排起了小队。"

  # 高好感线（≥70）：半告白
  - Condition: "{favorability_Fengxue} >= 70"
    Events:
      - "Type: Narration | Mode: Preset": "夜风轻拂，纸灯下的蓝发像海水。风雪侧过头，小声问：‘如果……吾辈把这次组队当成命运安排，你会不会觉得——太中二了？’"
      - "Type: Dialogue | Character: Fengxue | Mode: Prompt": "内心独白：鼓起勇气说一句暧昧而真诚的话，别太长。"
      - "Type: Player | Mode: Input": "（用一句话回应她。）"
      - "Type: Action | Tool: Set | Variable: player_last_input":
          Value: "{player_input}"
      - "Type: Notice | Mode: Preset | Location: popup": "【好感突破】风雪的眼睛亮晶晶的。你们默契对视，背景烟火正好绽放。"

  # 中好感线（50~69）：约定
  - Condition: "{favorability_Fengxue} >= 50 and {favorability_Fengxue} < 70"
    Events:
      - "Type: Narration | Mode: Preset": "人潮散去，摊位角落只剩折叠椅。风雪把猫爪手套拍在桌上：‘下次再来更帅一点的版本，如何？’"
      - "Type: Dialogue | Character: Fengxue | Mode: Prompt": "说一句略带中二而轻松的‘下次约定’台词。"

  # 低好感线（<50）：错身而过的可爱遗憾
  - Condition: "{favorability_Fengxue} < 50"
    Events:
      - "Type: Narration | Mode: Preset": "烟火把影子拉得很长。风雪挥了挥手：‘改天见，普通人类。’声音俏皮，却停在朋友的距离。"

EndCondition:
  Type: Linear
  NextUnitID: "99_End"
"""

unit_99_yaml_content = """
Events:
  - "Type: Notice | Mode: Preset | Location: popup": >
      【剧终】感谢游玩《风雪的校园恋爱喜剧》！\\n
      当前好感：{favorability_Fengxue}。\\n
      提示：多用真诚与尊重的语气、聊TA喜欢的话题，更容易推进甜线喔～
"""

# -----------------------------------------------------------------------------
# 2. 将文件路径与内容映射
# -----------------------------------------------------------------------------

files_to_create = {
    # 角色人设
    os.path.join("data", "characters", "Fengxue.yaml"): fengxue_yaml_content,
    os.path.join("data", "characters", "DM.yaml"): dm_yaml_content,

    # 故事包根目录文件
    os.path.join("story_packs", "campus_bluecat_romcom", "全局剧情配置.yaml"): global_config_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "剧情介绍.md"): intro_md_content,

    # 初始存档
    os.path.join("story_packs", "campus_bluecat_romcom", "save", "gamestate.yaml"): gamestate_yaml_content,
    
    # 剧情单元
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "00_Forced_Partners.yaml"): unit_00_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "01_Chapter_One_Kickoff.yaml"): unit_01_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "02A_Misunderstanding.yaml"): unit_02A_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "02B_Bonding.yaml"): unit_02B_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "03_Chapter_Three_Finale.yaml"): unit_03_yaml_content,
    os.path.join("story_packs", "campus_bluecat_romcom", "story", "99_End.yaml"): unit_99_yaml_content,
}

# -----------------------------------------------------------------------------
# 3. 循环创建目录和文件
# -----------------------------------------------------------------------------

total_files = len(files_to_create)
created_count = 0

for file_path, content in files_to_create.items():
    try:
        # 获取目录路径
        directory = os.path.dirname(file_path)
        
        # 如果目录不存在，则创建
        if not os.path.exists(directory):
            os.makedirs(directory)
            print(f"  📂 创建新目录: {directory}")
            
        # 写入文件
        # 使用 textwrap.dedent 来移除因 Python 代码缩进而导致的多余前导空格
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(textwrap.dedent(content).strip())
        
        print(f"  ✅ 成功写入: {file_path}")
        created_count += 1

    except Exception as e:
        print(f"  ❌ 写入失败: {file_path}")
        print(f"     错误原因: {e}")

# -----------------------------------------------------------------------------
# 4. 结束总结
# -----------------------------------------------------------------------------

print("\n----------------------------------------")
if created_count == total_files:
    print(f"🎉 部署完成！成功创建了 {created_count}/{total_files} 个文件。")
    print("现在你可以启动 NeoChat 并选择这个新的故事包了。")
    print("记得在角色选择界面为 Fengxue 和 DM 绑定对应的人设文件。")
else:
    print(f"⚠️ 部署部分完成。成功创建了 {created_count}/{total_files} 个文件。")
    print("请检查上面的错误信息。")
print("----------------------------------------")