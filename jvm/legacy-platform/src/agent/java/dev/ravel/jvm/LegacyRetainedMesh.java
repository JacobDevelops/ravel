package dev.ravel.jvm;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/** Retained completion transaction: no successful disposition means no native acknowledgement. */
final class LegacyRetainedMesh {
    interface Operations {
        byte[] copy(long generation, long key);
        /** False defers this completion without acknowledging native ownership. */
        boolean upload(byte[] copy, long generation, long key, long copyNanos);
        boolean accepts(long generation);
        long[] acknowledge(long generation, long key);
    }

    private LegacyRetainedMesh() { }

    static boolean drain(long generation, long[] metadata, int at, Operations operations) {
        if (generation <= 0 || metadata == null || at < 0 || at > metadata.length - 11
                || metadata[at] != generation || metadata[at + 1] <= 0)
            throw new IllegalStateException("Invalid retained completion identity");
        long key = metadata[at + 1];
        long started = System.nanoTime();
        byte[] copy = operations.copy(generation, key);
        long nanos = System.nanoTime() - started;
        if (copy == null || copy.length == 0 || !operations.accepts(generation)) return false;
        if (copy.length < 48 || copy.length > 1_376_304)
            throw new IllegalStateException("Native mesh copy exceeded its wire bounds");
        ByteBuffer header = ByteBuffer.wrap(copy).order(ByteOrder.LITTLE_ENDIAN);
        if (header.getInt(0) != 0x354d5652 || header.getInt(4) != 1
                || header.getLong(8) != generation || header.getLong(16) != key
                || header.getInt(24) < 0 || header.getInt(24) > 2
                || header.getInt(24) + 1L != metadata[at + 2]
                || Integer.toUnsignedLong(header.getInt(28)) != metadata[at + 7]
                || Integer.toUnsignedLong(header.getInt(32)) != metadata[at + 8]
                || header.getLong(40) != metadata[at + 10])
            throw new IllegalStateException("Retained mesh copy changed completion metadata");
        // Copied incomplete/error meshes and explicit capability refusals are dispositions, not draws.
        // Upload/publication exceptions are not dispositions and retain native ownership.
        // Transient oracle lock contention likewise retains ownership, but is not an exception.
        if (!operations.upload(copy, generation, key, nanos)) return false;
        if (!operations.accepts(generation)) return false;
        long[] acknowledgement = operations.acknowledge(generation, key);
        if (acknowledgement.length == 0) return false;
        if (acknowledgement.length != 1 || acknowledgement[0] != 1)
            throw new IllegalStateException("Retained completion was not acknowledged exactly once");
        return true;
    }
}
