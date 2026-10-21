package dev.ravel.legacy.opengl;

/** Render-thread confinement and monotonic retirement, independent of the GL driver. */
final class ProofLease {
    private long owner;
    private long retired;
    private long resident;

    void check(long generation, long expectedOwner) {
        if (expectedOwner <= 0 || Thread.currentThread().threadId() != expectedOwner
                || (owner != 0 && owner != expectedOwner))
            throw new IllegalStateException("Geometry proof requires the renderer/context owner thread");
        if (generation <= retired) throw new IllegalStateException("Retired geometry generation");
        owner = expectedOwner;
    }

    void uploaded(long generation, long expectedOwner) {
        check(generation, expectedOwner);
        resident = generation;
    }

    void invalidate(long generation, long expectedOwner) {
        check(generation, expectedOwner);
        resident = 0;
    }

    void draw(long generation, long expectedOwner) {
        check(generation, expectedOwner);
        if (resident != generation) throw new IllegalStateException("No complete mesh for this generation");
    }

    void retire(long generation, long expectedOwner) {
        if (owner != 0 && (owner != expectedOwner || Thread.currentThread().threadId() != owner))
            throw new IllegalStateException("Geometry cleanup requires its context owner thread");
        retired = Math.max(retired, generation);
        resident = 0;
    }
}
