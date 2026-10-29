package com.lingchat.maidbridge.voice;

import java.nio.ByteBuffer;
import javax.annotation.Nullable;
import javax.sound.sampled.AudioFormat;

/**
 * 最小的 WAV 解析：只认 16-bit PCM（单/双声道），给出 PCM 数据区间与播放格式。
 *
 * <p>不碰 Minecraft，也不复制音频数据——播放时由 {@link WavAudioStream} 按需从原字节里
 * 切片。宿主默认的 TTS 产物就是 16-bit PCM WAV；其余格式（mp3 / 24-bit / 浮点）一律拒绝，
 * 不做转码。
 */
public final class WavPcm {
    /** 单句语音上限：正常一句几百 KB，超过这个数多半是配置错了。 */
    public static final int MAX_BYTES = 8 * 1024 * 1024;

    private final byte[] data;
    private final int offset;
    private final int length;
    private final AudioFormat format;

    private WavPcm(byte[] data, int offset, int length, AudioFormat format) {
        this.data = data;
        this.offset = offset;
        this.length = length;
        this.format = format;
    }

    /** 解析失败（不是 RIFF/WAVE、不是 16-bit PCM、声道数不是 1/2）时返回 null。 */
    @Nullable
    public static WavPcm parse(byte[] bytes) {
        if (bytes == null || bytes.length < 12) {
            return null;
        }
        if (!matches(bytes, 0, "RIFF") || !matches(bytes, 8, "WAVE")) {
            return null;
        }
        int channels = 0;
        int sampleRate = 0;
        int bitsPerSample = 0;
        int dataOffset = -1;
        int dataLength = 0;
        int pos = 12;
        while (pos + 8 <= bytes.length) {
            // 按无符号读：流式写入的 WAV 会把 data 长度填成 0xFFFFFFFF。
            long declared = readIntLE(bytes, pos + 4) & 0xFFFFFFFFL;
            int payload = pos + 8;
            int available = bytes.length - payload;
            int chunkSize = (int) Math.min(declared, available);
            if (matches(bytes, pos, "fmt ") && chunkSize >= 16) {
                int audioFormat = readShortLE(bytes, payload);
                channels = readShortLE(bytes, payload + 2);
                sampleRate = readIntLE(bytes, payload + 4);
                bitsPerSample = readShortLE(bytes, payload + 14);
                if (audioFormat != 1) {
                    return null;
                }
            } else if (matches(bytes, pos, "data")) {
                dataOffset = payload;
                dataLength = chunkSize;
            }
            // 块按偶数字节对齐；越界或长度异常时中止，不把 pos 绕回去。
            long next = (long) payload + chunkSize + (chunkSize & 1);
            if (next <= pos) {
                break;
            }
            pos = (int) Math.min(next, Integer.MAX_VALUE);
        }
        if (dataOffset < 0 || dataLength <= 0) {
            return null;
        }
        if (channels != 1 && channels != 2) {
            return null;
        }
        if (sampleRate <= 0 || bitsPerSample != 16) {
            return null;
        }
        AudioFormat format = new AudioFormat(sampleRate, 16, channels, true, false);
        return new WavPcm(bytes, dataOffset, dataLength, format);
    }

    public AudioFormat format() {
        return this.format;
    }

    public int length() {
        return this.length;
    }

    /** 把从 {@code from} 开始的 PCM 字节放进 {@code out}，返回写入的字节数。 */
    public int copyTo(int from, ByteBuffer out) {
        if (from < 0 || from >= this.length) {
            return 0;
        }
        int count = Math.min(this.length - from, out.remaining());
        out.put(this.data, this.offset + from, count);
        return count;
    }

    private static boolean matches(byte[] bytes, int offset, String tag) {
        if (offset + tag.length() > bytes.length) {
            return false;
        }
        for (int i = 0; i < tag.length(); i++) {
            if (bytes[offset + i] != tag.charAt(i)) {
                return false;
            }
        }
        return true;
    }

    private static int readShortLE(byte[] bytes, int offset) {
        return (bytes[offset] & 0xFF) | ((bytes[offset + 1] & 0xFF) << 8);
    }

    private static int readIntLE(byte[] bytes, int offset) {
        return (bytes[offset] & 0xFF)
                | ((bytes[offset + 1] & 0xFF) << 8)
                | ((bytes[offset + 2] & 0xFF) << 16)
                | ((bytes[offset + 3] & 0xFF) << 24);
    }
}
