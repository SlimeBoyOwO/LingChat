package com.lingchat.maidbridge.voice;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.junit.jupiter.api.Test;

class TtsAssemblerTest {
    @Test
    void assemblesChunksOutOfOrder() {
        byte[] audio = wav(600_000);
        int chunk = TtsPayload.CHUNK_BYTES;
        int total = TtsPayload.chunkCount(audio.length);
        assertEquals(2, total);

        assertNull(TtsAssembler.accept(payload(1, 1, total, audio, chunk)));
        WavPcm wav = TtsAssembler.accept(payload(1, 0, total, audio, chunk));
        assertNotNull(wav);
        assertEquals(600_000, wav.length());
    }

    @Test
    void ignoresDuplicateChunk() {
        byte[] audio = wav(600_000);
        int chunk = TtsPayload.CHUNK_BYTES;
        int total = TtsPayload.chunkCount(audio.length);

        assertNull(TtsAssembler.accept(payload(2, 0, total, audio, chunk)));
        assertNull(TtsAssembler.accept(payload(2, 0, total, audio, chunk)));
        assertNotNull(TtsAssembler.accept(payload(2, 1, total, audio, chunk)));
    }

    @Test
    void rejectsMismatchedChunkCount() {
        byte[] audio = wav(600_000);
        assertNull(TtsAssembler.accept(new TtsPayload(
                3, 3, 0, 3, audio.length, Arrays.copyOfRange(audio, 0, TtsPayload.CHUNK_BYTES))));
    }

    @Test
    void chunkCountCoversBoundaries() {
        assertEquals(1, TtsPayload.chunkCount(1));
        assertEquals(1, TtsPayload.chunkCount(TtsPayload.CHUNK_BYTES));
        assertEquals(2, TtsPayload.chunkCount(TtsPayload.CHUNK_BYTES + 1));
        assertEquals(16, TtsPayload.chunkCount(WavPcm.MAX_BYTES));
    }

    private static TtsPayload payload(int transferId, int index, int total, byte[] audio, int chunk) {
        int from = index * chunk;
        int to = Math.min(audio.length, from + chunk);
        return new TtsPayload(1, transferId, index, total, audio.length, Arrays.copyOfRange(audio, from, to));
    }

    private static byte[] wav(int pcmLength) {
        ByteBuffer buffer = ByteBuffer.allocate(44 + pcmLength).order(ByteOrder.LITTLE_ENDIAN);
        putTag(buffer, "RIFF");
        buffer.putInt(36 + pcmLength);
        putTag(buffer, "WAVE");
        putTag(buffer, "fmt ");
        buffer.putInt(16);
        buffer.putShort((short) 1);
        buffer.putShort((short) 1);
        buffer.putInt(32000);
        buffer.putInt(64000);
        buffer.putShort((short) 2);
        buffer.putShort((short) 16);
        putTag(buffer, "data");
        buffer.putInt(pcmLength);
        return buffer.array();
    }

    private static void putTag(ByteBuffer buffer, String tag) {
        buffer.put(tag.getBytes(StandardCharsets.US_ASCII));
    }
}
