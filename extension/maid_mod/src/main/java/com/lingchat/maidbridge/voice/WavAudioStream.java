package com.lingchat.maidbridge.voice;

import java.nio.ByteBuffer;
import javax.sound.sampled.AudioFormat;
import net.minecraft.client.sounds.AudioStream;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.lwjgl.BufferUtils;

/**
 * 把 WAV 里的 PCM 数据当成 MC 的流式音源读给 OpenAL。
 *
 * <p>字节原样拷贝：WAV 的 16-bit PCM 是小端，而 {@link BufferUtils#createByteBuffer(int)}
 * 给的是本机字节序（桌面平台都是小端），所以不需要翻转。分块大小由声音引擎决定。
 */
@OnlyIn(Dist.CLIENT)
public final class WavAudioStream implements AudioStream {
    private final WavPcm wav;
    private int position;

    public WavAudioStream(WavPcm wav) {
        this.wav = wav;
    }

    @Override
    public AudioFormat getFormat() {
        return this.wav.format();
    }

    @Override
    public ByteBuffer read(int maxBytes) {
        if (this.position >= this.wav.length()) {
            return null;
        }
        int size = Math.min(maxBytes, this.wav.length() - this.position);
        ByteBuffer buffer = BufferUtils.createByteBuffer(size);
        this.position += this.wav.copyTo(this.position, buffer);
        buffer.flip();
        return buffer;
    }

    @Override
    public void close() {
        // 数据在内存里，没有需要释放的句柄。
    }
}
