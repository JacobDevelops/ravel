package dev.ravel.jvm;

final class LegacyRendererContract {
    static final int SNAPSHOT_MISMATCHES = 2;
    static final int SNAPSHOT_ADMITTED = 6;
    static final int BAKED_RECORD_BYTES = 232;
    static final int BAKED_MAX_QUADS = 8192;

    enum SnapshotFailure {
        GENERATION(1), LENGTHS(2), STATE(3), GEOMETRY(4), INCONSISTENT_GEOMETRY(5), PALETTE(6), NULL_INPUT(7);
        final long code;
        SnapshotFailure(long code) { this.code = code; }
        static SnapshotFailure decode(long code) {
            for (SnapshotFailure value : values()) if (value.code == code) return value;
            throw new IllegalStateException("Unknown native snapshot failure code");
        }
    }

    enum BakedFailure {
        BOUNDS(1), DECODE(2), EMISSION(3), ORACLE_HEADER(4), LAYER_COUNT(5), ORACLE_PAYLOAD(6), ALLOCATION(7);
        final long code;
        BakedFailure(long code) { this.code = code; }
        static BakedFailure decode(long code) {
            for (BakedFailure value : values()) if (value.code == code) return value;
            throw new IllegalStateException("Unknown native baked failure code");
        }
    }

    private LegacyRendererContract() { }
}
