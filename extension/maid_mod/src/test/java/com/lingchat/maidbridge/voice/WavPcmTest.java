package com.lingchat.maidbridge.voice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

class WavPcmTest {
    @Test
    void parses16BitPcm() {
        byte[] bytes = wav(1, 32000, 16, new byte[] {1, 2, 3, 4});
        WavPcm wav = WavPcm.parse(bytes);
        assertNotNull(wav);
        assertEquals(32000, (int) wav.format().getSampleRate());
        assertEquals(1, wav.format().getChannels());
        assertEquals(16, wav.format().getSampleSizeInBits());
        assertEquals(4, wav.length());
    }

    @Test
    void copiesPcmFromDataChunk() {
        byte[] bytes = wav(1, 16000, 16, new byte[] {7, 8, 9, 10, 11, 12});
        WavPcm wav = WavPcm.parse(bytes);
        assertNotNull(wav);
        ByteBuffer out = ByteBuffer.allocate(4);
        assertEquals(4, wav.copyTo(0, out));
        out.flip();
        assertEquals(7, out.get());
        assertEquals(8, out.get());
        assertEquals(9, out.get());
        assertEquals(10, out.get());
    }

    @Test
    void rejectsNonPcmAndUnsupportedDepths() {
        assertNull(WavPcm.parse(null));
        assertNull(WavPcm.parse(new byte[0]));
        assertNull(WavPcm.parse("not a wav file".getBytes(StandardCharsets.US_ASCII)));

        // fmt 里的 audioFormat 改成 3（IEEE float）
        byte[] floatWav = wav(1, 32000, 16, new byte[] {0, 0});
        floatWav[20] = 3;
        assertNull(WavPcm.parse(floatWav));

        // 8-bit 不支持
        assertNull(WavPcm.parse(wav(1, 32000, 8, new byte[] {0, 0})));

        // 三声道不支持
        assertNull(WavPcm.parse(wav(3, 32000, 16, new byte[] {0, 0, 0, 0, 0, 0})));
    }

    private static byte[] wav(int channels, int rate, int bits, byte[] pcm) {
        int blockAlign = channels * bits / 8;
        int riffSize = 36 + pcm.length;
        ByteBuffer buffer = ByteBuffer.allocate(44 + pcm.length).order(ByteOrder.LITTLE_ENDIAN);
        putTag(buffer, "RIFF");
        buffer.putInt(riffSize);
        putTag(buffer, "WAVE");
        putTag(buffer, "fmt ");
        buffer.putInt(16);
        buffer.putShort((short) 1);
        buffer.putShort((short) channels);
        buffer.putInt(rate);
        buffer.putInt(rate * blockAlign);
        buffer.putShort((short) blockAlign);
        buffer.putShort((short) bits);
        putTag(buffer, "data");
        buffer.putInt(pcm.length);
        buffer.put(pcm);
        return buffer.array();
    }

    private static void putTag(ByteBuffer buffer, String tag) {
        buffer.put(tag.getBytes(StandardCharsets.US_ASCII));
    }
}
