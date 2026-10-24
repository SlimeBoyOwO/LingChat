package com.lingchat.maidbridge.voice;

import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import javax.annotation.Nullable;

/**
 * 客户端侧的语音拼装：按传输 id 收齐分片，拼回整段 WAV。
 *
 * <p>分片在路上丢了就再也收不齐，所以每一路都有寿命，超时直接丢掉，避免半截数据常驻。
 */
public final class TtsAssembler {
    private static final long TIMEOUT_NANOS = 10_000_000_000L;
    private static final int MAX_TRANSFERS = 4;

    private static final Map<Integer, Pending> PENDING = new HashMap<>();

    private TtsAssembler() {
    }

    /** 收到一片；收齐且能解析成 WAV 时返回整段，否则返回 null。 */
    @Nullable
    static WavPcm accept(TtsPayload payload) {
        long now = System.nanoTime();
        prune(now);

        byte[] part = payload.data();
        int chunkLength = part.length;
        int total = payload.total();
        int totalBytes = payload.totalBytes();
        int index = payload.index();
        if (chunkLength <= 0 || chunkLength > TtsPayload.CHUNK_BYTES) {
            return null;
        }
        if (total <= 0 || total > TtsPayload.MAX_CHUNKS) {
            return null;
        }
        if (totalBytes <= 0 || totalBytes > WavPcm.MAX_BYTES) {
            return null;
        }
        if (total != TtsPayload.chunkCount(totalBytes)) {
            return null;
        }
        if (index < 0 || index >= total) {
            return null;
        }
        int offset = index * TtsPayload.CHUNK_BYTES;
        int expected = Math.min(TtsPayload.CHUNK_BYTES, totalBytes - offset);
        if (chunkLength != expected) {
            return null;
        }

        Pending pending = PENDING.get(payload.transferId());
        if (pending != null && !pending.matches(payload)) {
            PENDING.remove(payload.transferId());
            pending = null;
        }
        if (pending == null) {
            if (PENDING.size() >= MAX_TRANSFERS) {
                return null;
            }
            pending = new Pending(payload.entityId(), total, totalBytes);
            PENDING.put(payload.transferId(), pending);
        }
        if (pending.seen[index]) {
            return null;
        }
        pending.seen[index] = true;
        System.arraycopy(part, 0, pending.data, offset, chunkLength);
        pending.received++;
        pending.lastAt = now;
        if (pending.received == pending.total) {
            PENDING.remove(payload.transferId());
            return WavPcm.parse(pending.data);
        }
        return null;
    }

    private static void prune(long now) {
        Iterator<Map.Entry<Integer, Pending>> iterator = PENDING.entrySet().iterator();
        while (iterator.hasNext()) {
            if (now - iterator.next().getValue().lastAt > TIMEOUT_NANOS) {
                iterator.remove();
            }
        }
    }

    private static final class Pending {
        private final int entityId;
        private final int total;
        private final int totalBytes;
        private final boolean[] seen;
        private final byte[] data;
        private int received;
        private long lastAt = System.nanoTime();

        private Pending(int entityId, int total, int totalBytes) {
            this.entityId = entityId;
            this.total = total;
            this.totalBytes = totalBytes;
            this.seen = new boolean[total];
            this.data = new byte[totalBytes];
        }

        private boolean matches(TtsPayload payload) {
            return payload.entityId() == entityId
                    && payload.total() == total
                    && payload.totalBytes() == totalBytes;
        }
    }
}
