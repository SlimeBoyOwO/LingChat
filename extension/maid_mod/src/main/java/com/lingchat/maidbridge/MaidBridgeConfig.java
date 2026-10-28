package com.lingchat.maidbridge;

import net.neoforged.neoforge.common.ModConfigSpec;

/** 模组配置。默认值与社区桥接模组保持一致，插件端不用改。 */
public final class MaidBridgeConfig {
    public static final ModConfigSpec SPEC;
    public static final ModConfigSpec.BooleanValue ENABLED;
    public static final ModConfigSpec.IntValue PORT;
    public static final ModConfigSpec.BooleanValue CHAT_BUBBLE_ENABLED;
    public static final ModConfigSpec.BooleanValue CHAT_BOX_ENABLED;
    public static final ModConfigSpec.BooleanValue PERCEPTION_ENABLED;
    public static final ModConfigSpec.IntValue PERCEPTION_INTERVAL_TICKS;
    public static final ModConfigSpec.DoubleValue LOW_HEALTH_RATIO;
    public static final ModConfigSpec.BooleanValue NOTICE_ENABLED;
    public static final ModConfigSpec.IntValue NOTICE_RADIUS;
    public static final ModConfigSpec.IntValue HEARD_RADIUS;

    static {
        ModConfigSpec.Builder builder = new ModConfigSpec.Builder();
        builder.comment("LingChat 女仆桥接").push("bridge");

        ENABLED = builder
                .comment("总开关。关掉后不再监听端口，用于排查是模组的问题还是插件的问题。")
                .define("enabled", true);
        PORT = builder
                .comment("WebSocket 端口，只监听 127.0.0.1。")
                .defineInRange("port", 48920, 1024, 65535);
        CHAT_BUBBLE_ENABLED = builder
                .comment("把角色的话显示成女仆头顶的聊天气泡。")
                .define("chatBubbleEnabled", true);
        CHAT_BOX_ENABLED = builder
                .comment("把角色的话同时写进聊天栏，只有女仆的主人看得到。")
                .define("chatBoxEnabled", true);
        PERCEPTION_ENABLED = builder
                .comment("把游戏世界的变化（换地形、天黑、下雨、受伤这类）推给 LingChat，让它记进台词表。")
                .define("perceptionEnabled", true);
        PERCEPTION_INTERVAL_TICKS = builder
                .comment("多久采一次样（tick，20 tick = 1 秒）。只在变化时才推，不是每次都推。")
                .defineInRange("perceptionIntervalTicks", 100, 20, 1200);
        LOW_HEALTH_RATIO = builder
                .comment("血量低于这个比例时报一次「快撑不住了」；回到阈值 ×1.2 以上才允许再报。")
                .defineInRange("lowHealthRatio", 0.30, 0.05, 0.9);
        NOTICE_ENABLED = builder
                .comment("把身边的变化（主人走远、有东西靠近、进洞变暗、踩进水里、玩家给她东西）"
                        + "也推给 LingChat。关掉只影响「她自发注意到」，不影响玩家问「你周围有什么」，"
                        + "也不影响潜行右键投喂本身。")
                .define("noticeEnabled", true);
        NOTICE_RADIUS = builder
                .comment("身边这一层扫多大范围（格）。越大越费，别超过 24。")
                .defineInRange("noticeRadius", 12, 4, 24);
        HEARD_RADIUS = builder
                .comment("看不见的活物只在这个距离内按声音感知（格）。看得见的另按工作范围算。")
                .defineInRange("heardRadius", 6, 0, 16);
        builder.pop();
        SPEC = builder.build();
    }

    private MaidBridgeConfig() {
    }
}
