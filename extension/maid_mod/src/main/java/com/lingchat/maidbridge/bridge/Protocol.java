package com.lingchat.maidbridge.bridge;


/**
 * 桥接协议里的消息类型。字段名与《车万女仆》社区桥接模组保持一致，
 * 这样 LingChat 的插件端不用为换成我们自己的模组做任何改动。
 */
public final class Protocol {
    /** 上行：插件 → 模组 */
    public static final String PING = "ping";
    public static final String GET_MAID_STATUS = "get_maid_status";
    /** 问她现在是死的还是活的（死亡是持久状态，死后实体已经不在世上）。 */
    public static final String MAID_STATE = "maid_state";
    public static final String SET_MONITORED_MAID = "set_monitored_maid";
    public static final String SEND_CHAT = "send_chat";
    public static final String MAID_ACTION = "maid_action";
    /** 问一次「她身边现在是什么样」——按需查询，不参与周期采样。 */
    public static final String MAID_LOOK_AROUND = "maid_look_around";
    /** 给她换一个工作模式（农场、钓鱼、攻击、空闲……）。 */
    public static final String MAID_SET_TASK = "maid_set_task";
    /** 让她去打指定的那个活物（主人也算可点名）。 */
    public static final String MAID_ATTACK = "maid_attack";
    /** 让她去挖方块：玩家正看着的那块，或附近的某一种方块。 */
    public static final String MAID_MINE = "maid_mine";
    /** 让她把一块方块放到玩家指着的那一面，或一个空格。 */
    public static final String MAID_PLACE = "maid_place";
    /** 把她背包里的东西交给主人。 */
    public static final String MAID_GIVE_ITEM = "maid_give_item";
    /** 把背包里的东西戴到她头上（最后一格是头饰栏），或把头上那件摘下来。 */
    public static final String MAID_HEADWEAR = "maid_headwear";
    /** 在女仆的位置播放一句 TTS 语音（WAV 的 base64）。 */
    public static final String PLAY_TTS = "play_tts";
    /** 这一轮已经说完话了：动作后的游荡冷却可以收了。 */
    public static final String MAID_SETTLE = "maid_settle";
    /** 她最喜欢吃的东西（角色设定在 LingChat 那边），主人拿在手上时她会凑过来讨。 */
    public static final String SET_FAVORITE_FOOD = "set_favorite_food";

    /** 下行：模组 → 插件 */
    public static final String PONG = "pong";
    public static final String MAID_STATUS = "maid_status";
    /** 她死活状态的回执。 */
    public static final String MAID_STATE_RESULT = "maid_state_result";
    public static final String CONFIG = "config";
    public static final String CHAT_RESULT = "chat_result";
    public static final String CHAT_MESSAGE = "chat_message";
    public static final String MAID_ACTION_RESULT = "maid_action_result";
    /** 她身边的一份快照。本条是本模组的扩展帧，社区那套桥接不一定认。 */
    public static final String MAID_LOOK_AROUND_RESULT = "maid_look_around_result";
    /** 换工作模式的回执。 */
    public static final String MAID_SET_TASK_RESULT = "maid_set_task_result";
    /** 攻击指令的回执。 */
    public static final String MAID_ATTACK_RESULT = "maid_attack_result";
    /** 挖掘指令的回执。 */
    public static final String MAID_MINE_RESULT = "maid_mine_result";
    /** 放置指令的回执。 */
    public static final String MAID_PLACE_RESULT = "maid_place_result";
    /** 交物品的回执。 */
    public static final String MAID_GIVE_ITEM_RESULT = "maid_give_item_result";
    /** 戴/摘头饰的回执。 */
    public static final String MAID_HEADWEAR_RESULT = "maid_headwear_result";
    /** 游荡冷却收掉的回执。 */
    public static final String MAID_SETTLE_RESULT = "maid_settle_result";
    /** 游戏世界的事件（换地形、天黑、下雨、受伤这类）。 */
    public static final String EVENT = "event";
    public static final String ERROR = "error";

    private Protocol() {
    }
}
