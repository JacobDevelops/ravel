package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.util.IdentityHashMap;

final class LegacyVanillaBuffers {
    private static final IdentityHashMap<Object, Long> BYTES = new IdentityHashMap<>();
    private static Field id;
    private static Field format;
    private static Object block;
    private static long live;
    private static long peak;

    private LegacyVanillaBuffers() { }

    private static void initialize() throws ReflectiveOperationException {
        if (id != null) return;
        Class<?> type = LegacyAccess.type("LEGACY_VERTEX_BUFFER");
        format = Members.field(type, LegacyNames.member("LEGACY_VERTEX_BUFFER", "format"));
        block = Members.field(LegacyAccess.type("LEGACY_VERTEX_FORMATS"),
                LegacyNames.member("LEGACY_VERTEX_FORMATS", "BLOCK")).get(null);
        id = Members.field(type, LegacyNames.member("LEGACY_VERTEX_BUFFER", "id"));
    }

    static synchronized void upload(Object buffer, Object bytes) {
        try {
            initialize();
            if (format.get(buffer) != block) return;
            long size = ((ByteBuffer) bytes).remaining();
            Long previous = BYTES.put(buffer, size);
            live += size - (previous == null ? 0 : previous);
            peak = Math.max(peak, live);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Vanilla section buffer accounting failed", failure);
        }
    }

    static synchronized void deleted(Object buffer) {
        if (id == null) return;
        try {
            if (id.getInt(buffer) >= 0) return;
            Long previous = BYTES.remove(buffer);
            if (previous != null) live -= previous;
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Vanilla section buffer retirement failed", failure);
        }
    }

    static synchronized long[] metrics() { return new long[] {live, peak, BYTES.size()}; }
}
