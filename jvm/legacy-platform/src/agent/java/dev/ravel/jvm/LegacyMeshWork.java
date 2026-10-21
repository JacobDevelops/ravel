package dev.ravel.jvm;

import java.util.Arrays;

/**
 * Java-owned admission keys for native geometry jobs. Default polls drain diagnostic metadata only;
 * explicitly selected geometric proof retains a completion until bounded Java copy/upload and ack.
 * Every table and sample buffer is fixed-size, independent of world size or session length.
 */
final class LegacyMeshWork {
    static final int POLL = 0;
    static final int STATS = 1;
    static final int SHUTDOWN = 2;
    static final int RETIRED = 3;
    static final int HEADER = 4;
    static final int RECORD = 11;
    static final int MAX_RECORDS = 8;
    static final int STATS_WIDTH = 18;
    static final int SLOTS = 16;
    static final int SAMPLE_LIMIT = 4096;
    static final long ALL_VISIBLE = (1L << 36) - 1;
    // Sixteen-cube interior, six quads of four vertices each: an upper bound, not a mesh estimate.
    static final long MAX_VERTICES = 16L * 16 * 16 * 6 * 4;
    static final String[] NAMES = {"submitted", "refused", "tableFull", "drained", "complete",
            "incomplete", "errors", "unsupportedKeys", "maxUnsupportedKeys", "vertices", "indices",
            "outputBytes", "maxOutputBytes", "allocations", "maxAllocations", "nonEmptyMeshes",
            "occludingMeshes", "rejectedRecords", "retiredBeforeDrain", "stalePolls", "polls",
            "maxRecordsPerPoll", "firstQueueNanos", "firstMeshNanos", "firstDrainNanos",
            "firstCaptureNanos", "maxQueueNanos", "maxMeshNanos", "maxDrainNanos", "maxCaptureNanos",
            "drainChecks", "drainFailures", "maxOutstanding", "maxReservedBytes", "maxOutstandingLimit",
            "maxReservedBytesLimit", "frameSamples", "maxFrameNanos", "firstFiveSecondsMaxFrameNanos",
            "stressSubmitted", "stressDrained", "maxTableOccupancy", "injectedStaleRecordRejections",
            "captureFailures", "lastFailureCode", "lastFailureDetailA", "lastFailureDetailB",
            "lastFailureDetailC", "workerFailures", "lifecyclesWithoutRetiredPool", "retiredUnadmitted",
            "lastRetiredPoolGeneration"};
    static final String[] SERIES = {"queue", "mesh", "drain", "frame"};
    // Slot states. A reservation is never freed while its native call may still return a receipt.
    private static final byte FREE = 0;
    private static final byte RESERVED = 1;
    private static final byte ADMITTED = 2;
    private static final byte DRAINED = 3;
    private static final byte RETIRED_PENDING = 4;
    private static final long FIRST_WINDOW_NANOS = 5_000_000_000L;
    private static final long[] KEYS = new long[SLOTS];
    private static final long[] GENERATIONS = new long[SLOTS];
    private static final long[] RESERVED_AT = new long[SLOTS];
    private static final byte[] STATES = new byte[SLOTS];
    private static final long[] METRICS = new long[NAMES.length];
    private static final long[][] SAMPLES = new long[SERIES.length][SAMPLE_LIMIT];
    private static final int[] SAMPLED = new int[SERIES.length];
    private static long nextKey;
    private static long stressStarted;
    private static long lastFrame;
    // Highest generation whose Java admission is closed; any later lifecycle generation reopens it.
    private static long closedGeneration;
    private static final long SETTLE_NANOS = 10_000_000_000L;

    private LegacyMeshWork() { }

    /**
     * Reserves a monotonic key before native admission so an early completion still matches. Returns
     * zero for a full table or a closed generation; callers treat zero as a clean refusal without JNI.
     */
    static synchronized long reserve(long generation) {
        if (generation <= closedGeneration) return 0;
        int free = find(0);
        if (free < 0) {
            METRICS[2]++;
            return 0;
        }
        KEYS[free] = ++nextKey;
        GENERATIONS[free] = generation;
        RESERVED_AT[free] = System.nanoTime();
        STATES[free] = RESERVED;
        METRICS[41] = Math.max(METRICS[41], occupied());
        return KEYS[free];
    }

    /** A key for batches expected to be refused; it never enters the table. */
    static synchronized long probeKey() { return ++nextKey; }

    static synchronized int occupied() {
        int count = 0;
        for (long key : KEYS) if (key != 0) count++;
        return count;
    }

    /** Observes whether lifecycle retirement is waiting for this reservation's admission receipt. */
    static synchronized boolean retiredPending(long key) {
        int slot = find(key);
        return slot >= 0 && STATES[slot] == RETIRED_PENDING;
    }

    /**
     * Settles a reservation exactly once. Admission is only {@code [1,..,1,key]}; a refusal
     * {@code [1,..,0,key]}, a diagnostic failure {@code [-1,code,a,b,c,0,0,key]}, an empty (stale/busy)
     * or absent receipt release the key and are never counted as admitted.
     */
    static synchronized void commit(long key, long[] receipt, long captureNanos) {
        int slot = find(key);
        if (slot < 0) throw new IllegalStateException("Java admission key was lost before settlement: " + key);
        boolean receipted = receipt != null && receipt.length == 8;
        if ((receipt != null && receipt.length != 0 && !receipted) || (receipted && (receipt[7] != key
                || (receipt[0] != 1 && receipt[0] != -1) || (receipt[6] & ~1L) != 0
                || (receipt[0] == -1 && receipt[6] != 0)))) {
            release(slot);
            throw new IllegalStateException("Native admission receipt changed Java key " + key
                    + ": " + Arrays.toString(receipt));
        }
        byte state = STATES[slot];
        if (!receipted || receipt[0] != 1 || receipt[6] != 1) {
            release(slot);
            if (state == DRAINED) throw new IllegalStateException("Native completed unadmitted key " + key);
            if (state == RETIRED_PENDING) METRICS[50]++;
            if (receipted && receipt[0] == -1) failure(receipt);
            else if (receipted) METRICS[1]++;
            return;
        }
        admitted(captureNanos);
        if (state == RESERVED) STATES[slot] = ADMITTED;
        else {
            release(slot);
            // Admitted before the lifecycle advanced, then discarded by native invalidation undrained.
            if (state == RETIRED_PENDING) METRICS[18]++;
        }
    }

    private static void admitted(long captureNanos) {
        METRICS[0]++;
        if (stressStarted != 0) METRICS[39]++;
        if (METRICS[0] == 1) METRICS[25] = captureNanos;
        METRICS[29] = Math.max(METRICS[29], captureNanos);
    }

    private static void failure(long[] receipt) {
        METRICS[43]++;
        METRICS[44] = receipt[1];
        METRICS[45] = receipt[2];
        METRICS[46] = receipt[3];
        METRICS[47] = receipt[4];
    }

    /** Owner-thread scheduling seam; the native call holds no Java monitor. */
    static void poll() {
        if (Thread.currentThread().threadId() != LegacyRenderer.ownerThread()) return;
        long generation = LegacyRenderer.currentGeneration();
        boolean retained = LegacyGeometryProof.selected();
        long[] response = Hook.ravelRendererWork(retained ? 4 : POLL, generation, 0);
        if (retained && response.length != 0) {
            retainedPoll(response, generation);
            response = new long[0];
        }
        long[] stats = LegacyRuntime.PROOF ? Hook.ravelRendererWork(STATS, generation, 0) : null;
        synchronized (LegacyMeshWork.class) {
            retireLocked(generation);
            if (response.length == 0) {
                if (!retained) METRICS[19]++;
            } else drainLocked(response, generation);
            if (stats != null && stats.length != 0) {
                limits(stats);
                if (stats[1] != generation) throw new IllegalStateException("Native stats changed generation");
            }
        }
    }

    /**
     * Lifecycle seam after reload/unload/close. Native invalidation joined the retired pool before the
     * event returned; operation 3, authorized by the current generation, reports that pool's actual final
     * stats with its own generation. Only a pool of exactly the retired generation counts as drain proof.
     */
    static void retire(long expected) {
        long current = LegacyRenderer.currentGeneration();
        long[] retired = current != expected ? Hook.ravelRendererWork(RETIRED, current, 0) : new long[0];
        synchronized (LegacyMeshWork.class) {
            retireLocked(current);
            if (current == expected) return;
            if (retired.length == 0) {
                METRICS[49]++;
                return;
            }
            limits(retired);
            if (retired[1] < expected) METRICS[49]++; // An older pool: this generation never admitted work.
            else if (retired[1] != expected || retired[2] != 1 || retired[14] != 0 || retired[17] != 0
                    || Arrays.stream(retired, 3, 8).anyMatch(value -> value != 0)) METRICS[31]++;
            else {
                METRICS[30]++;
                METRICS[51] = retired[1];
            }
        }
    }

    /** One mesh per owner poll: bounded copy/upload work even when eight completions are retained. */
    private static void retainedPoll(long[] response, long generation) {
        if (!retainedAdmission(response, generation)) return;
        if (!LegacyGeometryProof.transfer(generation, response, HEADER)) return;
        synchronized (LegacyMeshWork.class) {
            if (!accept(response, HEADER, generation)) METRICS[17]++;
        }
    }

    /** Admission gate is independent of JNI/GL, and never admits more than the first retained record. */
    static boolean retainedAdmission(long[] response, long generation) {
        int count = pollCount(response, generation);
        synchronized (LegacyMeshWork.class) {
            retireLocked(generation);
            METRICS[20]++;
            METRICS[21] = Math.max(METRICS[21], Math.min(count, 1));
            if (count == 0) return false;
            validate(response, HEADER);
            int slot = find(response[HEADER + 1]);
            if (slot < 0 || GENERATIONS[slot] != generation
                    || (STATES[slot] != RESERVED && STATES[slot] != ADMITTED))
                throw new IllegalStateException("Retained completion lacks Java admission");
            return true;
        }
    }

    private static void drainLocked(long[] response, long generation) {
        int count = pollCount(response, generation);
        METRICS[20]++;
        METRICS[21] = Math.max(METRICS[21], count);
        for (int index = 0; index < count; index++)
            if (!accept(response, HEADER + index * RECORD, generation)) METRICS[17]++;
    }

    private static int pollCount(long[] response, long generation) {
        if (response.length < HEADER || response[0] != 1 || response[1] != generation
                || response[3] != RECORD || response[2] < 0 || response[2] > MAX_RECORDS
                || response.length != HEADER + response[2] * RECORD)
            throw new IllegalStateException("Malformed native geometry poll: " + Arrays.toString(response));
        return (int) response[2];
    }

    static boolean accept(long[] record, int at, long current) {
        long generation = record[at];
        int slot = record[at + 1] > 0 ? find(record[at + 1]) : -1;
        if (slot < 0 || generation != current || GENERATIONS[slot] != generation
                || (STATES[slot] != RESERVED && STATES[slot] != ADMITTED)) return false;
        validate(record, at);
        long drainNanos = System.nanoTime() - RESERVED_AT[slot];
        // A completion can overtake its own receipt; commit then releases the drained slot.
        if (STATES[slot] == RESERVED) STATES[slot] = DRAINED;
        else release(slot);
        tally(record, at, drainNanos);
        return true;
    }

    private static void validate(long[] r, int at) {
        long status = r[at + 2], vertices = r[at + 7], indices = r[at + 8], unsupported = r[at + 9];
        boolean shape = vertices >= 0 && vertices <= MAX_VERTICES && vertices % 4 == 0
                && indices == vertices / 4 * 6 && r[at + 10] >= 0 && r[at + 10] <= ALL_VISIBLE;
        boolean costs = r[at + 3] >= 0 && r[at + 4] >= 0 && r[at + 5] >= 0 && r[at + 6] >= 0;
        boolean outcome = switch ((int) status) {
            case 1 -> unsupported == 0 && r[at + 5] > 0;
            case 2 -> unsupported > 0 && unsupported <= 18 * 18 * 18 && r[at + 5] > 0;
            case 3 -> vertices == 0 && unsupported == 0 && r[at + 10] == 0;
            default -> false;
        };
        if (status != (int) status || !shape || !costs || !outcome)
            throw new IllegalStateException("Native geometry metadata violated its contract: "
                    + Arrays.toString(Arrays.copyOfRange(r, at, at + RECORD)));
    }

    private static void tally(long[] r, int at, long drainNanos) {
        long status = r[at + 2];
        METRICS[3]++;
        METRICS[(int) status + 3]++;
        METRICS[7] += r[at + 9];
        METRICS[8] = Math.max(METRICS[8], r[at + 9]);
        METRICS[9] += r[at + 7];
        METRICS[10] += r[at + 8];
        METRICS[11] += r[at + 5];
        METRICS[12] = Math.max(METRICS[12], r[at + 5]);
        METRICS[13] += r[at + 6];
        METRICS[14] = Math.max(METRICS[14], r[at + 6]);
        if (r[at + 7] > 0) METRICS[15]++;
        if (status != 3 && r[at + 10] != ALL_VISIBLE) METRICS[16]++;
        if (stressStarted != 0) METRICS[40]++;
        if (METRICS[3] == 1) {
            METRICS[22] = r[at + 3];
            METRICS[23] = r[at + 4];
            METRICS[24] = drainNanos;
        }
        METRICS[26] = Math.max(METRICS[26], r[at + 3]);
        METRICS[27] = Math.max(METRICS[27], r[at + 4]);
        METRICS[28] = Math.max(METRICS[28], drainNanos);
        sample(0, r[at + 3]);
        sample(1, r[at + 4]);
        sample(2, drainNanos);
    }

    private static void limits(long[] stats) {
        if (stats.length != STATS_WIDTH || stats[0] != 1 || stats[1] <= 0 || (stats[17] & ~1L) != 0
                || (stats[2] & ~1L) != 0 || (stats[14] & ~1L) != 0 || stats[15] <= 0 || stats[16] <= 0
                || stats[3] > stats[15] || stats[8] > stats[15] || stats[7] > stats[16] || stats[9] > stats[16])
            throw new IllegalStateException("Native worker stats exceeded their limits: " + Arrays.toString(stats));
        METRICS[32] = Math.max(METRICS[32], stats[8]);
        METRICS[33] = Math.max(METRICS[33], stats[9]);
        METRICS[34] = stats[15];
        METRICS[35] = stats[16];
        METRICS[48] = Math.max(METRICS[48], stats[17]);
    }

    private static void retireLocked(long generation) {
        for (int slot = 0; slot < SLOTS; slot++)
            if (KEYS[slot] != 0 && GENERATIONS[slot] != generation) retireSlot(slot);
    }

    // Only admitted, undrained keys are cancellations; pending reservations settle at their commit.
    private static void retireSlot(int slot) {
        if (STATES[slot] == ADMITTED) {
            METRICS[18]++;
            release(slot);
        } else if (STATES[slot] == RESERVED) {
            STATES[slot] = RETIRED_PENDING;
        }
    }

    private static int find(long key) {
        for (int slot = 0; slot < SLOTS; slot++) if (KEYS[slot] == key) return slot;
        return -1;
    }

    // Every caller holds the class monitor; a shutdown may be waiting for this settlement.
    private static void release(int slot) {
        KEYS[slot] = 0;
        GENERATIONS[slot] = 0;
        RESERVED_AT[slot] = 0;
        STATES[slot] = FREE;
        LegacyMeshWork.class.notifyAll();
    }

    private static void sample(int series, long nanos) {
        if (SAMPLED[series] < SAMPLE_LIMIT) SAMPLES[series][SAMPLED[series]++] = nanos;
    }

    /** Observational frame-to-frame interval during the armed stress window; not mesh attribution. */
    static synchronized void frame() {
        if (stressStarted == 0) return;
        long now = System.nanoTime();
        if (lastFrame != 0) {
            long interval = now - lastFrame;
            METRICS[36]++;
            METRICS[37] = Math.max(METRICS[37], interval);
            if (lastFrame - stressStarted <= FIRST_WINDOW_NANOS) METRICS[38] = Math.max(METRICS[38], interval);
            sample(3, interval);
        }
        lastFrame = now;
    }

    static synchronized void beginStress() {
        stressStarted = System.nanoTime();
        lastFrame = 0;
    }

    static synchronized void endStress() {
        stressStarted = 0;
        lastFrame = 0;
    }

    /**
     * Actual native stale refusals, then a forged record carrying a really reserved current key and the
     * retired generation: only the generation check can reject it. The probe reservation is withdrawn.
     */
    static boolean injectStaleProof(long retired) {
        long current = LegacyRenderer.currentGeneration();
        if (retired <= 0 || retired >= current) return false;
        if (Hook.ravelRendererWork(POLL, retired, 0).length != 0
                || Hook.ravelRendererWork(STATS, retired, 0).length != 0
                || Hook.ravelRendererWork(RETIRED, retired, 0).length != 0)
            throw new IllegalStateException("Native served a retired generation: " + retired);
        synchronized (LegacyMeshWork.class) {
            long key = reserve(current);
            int slot = key == 0 ? -1 : find(key);
            if (slot < 0) return false;
            long drained = METRICS[3];
            long[] forged = {retired, key, 1, 0, 1, 1, 0, 0, 0, 0, ALL_VISIBLE};
            boolean published = accept(forged, 0, current);
            boolean held = STATES[slot] == RESERVED && KEYS[slot] == key;
            release(slot);
            if (published || !held || METRICS[3] != drained)
                throw new IllegalStateException("Retired geometry record was published");
            METRICS[42]++;
        }
        return true;
    }

    /**
     * Terminal drain. Java admission for the current generation closes under this monitor before native
     * operation 2, so no new key can follow a native close. After native joins, every slot is withdrawn,
     * then the monitor waits (released, never across JNI) for in-flight receipts to settle their keys.
     */
    static long[] shutdown() {
        long generation = LegacyRenderer.currentGeneration();
        synchronized (LegacyMeshWork.class) {
            closedGeneration = Math.max(closedGeneration, generation);
        }
        long[] stats = Hook.ravelRendererWork(SHUTDOWN, generation, 0);
        synchronized (LegacyMeshWork.class) {
            for (int slot = 0; slot < SLOTS; slot++) if (KEYS[slot] != 0) retireSlot(slot);
            awaitSettlement(System.nanoTime() + SETTLE_NANOS);
            if (stats.length != 0) {
                limits(stats);
                if (stats[1] != generation) throw new IllegalStateException("Native shutdown changed generation");
            }
            return stats;
        }
    }

    // Caller holds the monitor; wait releases it so capture threads can commit their receipts.
    private static void awaitSettlement(long deadline) {
        while (occupied() != 0) {
            long remaining = deadline - System.nanoTime();
            if (remaining <= 0)
                throw new IllegalStateException("Java admission keys unsettled after native close: " + occupied());
            try {
                LegacyMeshWork.class.wait(Math.max(1, remaining / 1_000_000));
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                throw new IllegalStateException("Interrupted awaiting admission key settlement", interrupted);
            }
        }
    }

    static synchronized long[] metrics() { return METRICS.clone(); }

    static synchronized long percentile(int series, int percent) {
        int count = SAMPLED[series];
        if (count == 0) return 0;
        long[] sorted = Arrays.copyOf(SAMPLES[series], count);
        Arrays.sort(sorted);
        return sorted[(count * percent + 99) / 100 - 1];
    }
}
