package com.lingchat.maidbridge.voice;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.init.InitSounds;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.resources.sounds.EntityBoundSoundInstance;
import net.minecraft.client.resources.sounds.Sound;
import net.minecraft.client.sounds.AudioStream;
import net.minecraft.client.sounds.SoundBufferLibrary;
import net.minecraft.sounds.SoundSource;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

/**
 * 绑定在女仆身上的一句 TTS 语音。
 *
 * <p>复用《车万女仆》已注册的 {@code maid.ai_chat}（sounds.json 里是 {@code stream: true}）：
 * 声音引擎照常建立流式通道、按实体位置更新坐标并做距离衰减，只是音频内容换成我们自己的
 * PCM 流。这也是 TLM 自己播放 AI 语音的方式，只是它只认 mp3 / ogg。
 */
@OnlyIn(Dist.CLIENT)
public final class TtsSoundInstance extends EntityBoundSoundInstance {
    /** 声音引擎按 max(音量, 1) 乘以声音自带的 16 格衰减距离，4 对应约 64 格的可听范围。 */
    private static final float VOLUME = 4f;

    private final WavPcm wav;

    public TtsSoundInstance(EntityMaid maid, WavPcm wav) {
        super(InitSounds.MAID_AI_CHAT.get(), SoundSource.VOICE, VOLUME, 1f, maid, maid.getId());
        this.wav = wav;
    }

    @Override
    public CompletableFuture<AudioStream> getStream(
            SoundBufferLibrary library, Sound sound, boolean looping) {
        return CompletableFuture.completedFuture(new WavAudioStream(this.wav));
    }
}
