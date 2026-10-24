package com.lingchat.maidbridge.action.task;

import java.util.Locale;
import net.minecraft.resources.ResourceLocation;

/**
 * 工作模式的 id 归一：模型给 {@code farm} 与 {@code touhou_little_maid:farm} 是同一件事。
 *
 * <p>纯逻辑，不查注册表——「有没有这个模式」由 {@code TaskManager} 回答，所以模组或
 * 附属包加进来的任务也照样能切，不必在这里维护一张白名单。
 */
public final class MaidTaskIds {
    /** 车万女仆自己的命名空间：只写短名时补它。 */
    public static final String NAMESPACE = "touhou_little_maid";

    private MaidTaskIds() {
    }

    /** 认不出的写法给 null，由调用方转成拒绝原因。 */
    public static ResourceLocation resolve(String raw) {
        String text = raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
        if (text.isEmpty()) {
            return null;
        }
        if (!text.contains(":")) {
            text = NAMESPACE + ":" + text;
        }
        ResourceLocation id = ResourceLocation.tryParse(text);
        // 1.21 的 tryParse 认「命名空间:」这种空路径，那不可能是任务
        return id == null || id.getPath().isEmpty() ? null : id;
    }
}
