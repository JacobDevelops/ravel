package dev.ravel.jvm;

import java.util.WeakHashMap;
import java.lang.reflect.Field;

/** Observes vanilla work; only owned numeric batches cross the native boundary. */
public final class LegacyRenderer {
    private static final int TASK_LIMIT = 4096;
    // The pinned ChunkCompileTask inherits Object equality/hashCode; weak keys retain no game task.
    private static final WeakHashMap<Object, Long> TASKS = new WeakHashMap<>();
    private static final long[] COUNTS = new long[10];
    private static volatile long owner;
    private static volatile boolean workerObserved;
    private static Field completionTask;
    private static long staleReturns;
    // Acceptance alone retains one real task/association until its injected stale observation.
    private static boolean proofArmed;
    private static Object proofTask;
    private static boolean proofTaskFinished;

    private LegacyRenderer() { }

    static void enter(String id, Object first) {
        switch (id) {
            case "legacyResourceReload" -> ownerEvent(1);
            case "legacyRendererUnload" -> ownerEvent(2);
            case "legacyRendererClose" -> ownerEvent(2);
            case "legacyRendererFrame" -> ownerEvent(3);
            case "legacyRendererTerrain" -> ownerEvent(4);
            case "legacyRendererLayer" -> ownerEvent(5);
            case "legacyRendererUpdate" -> ownerEvent(6);
            case "legacyRendererUpload" -> ownerEvent(9);
            default -> { }
        }
    }

    // These returns can include aborted compilation or a cancelled callback's early return.
    // Generation admission is not a compilation/publication outcome claim.
    static void exit(String id, Object receiver, Object first) throws ReflectiveOperationException {
        switch (id) {
            case "legacyRendererTaskCreate" -> capture(receiver);
            case "legacyRendererCompile" -> compileReturn(receiver, first);
            case "legacyRendererWorkerComplete" -> callbackReturn(receiver);
            default -> { }
        }
    }

    private static void compileReturn(Object chunk, Object task) throws ReflectiveOperationException {
        Long generation;
        boolean retainProof;
        synchronized (LegacyRenderer.class) {
            generation = TASKS.get(task);
            retainProof = LegacyRuntime.PROOF && proofArmed;
        }
        normalReturn(7, task, false);
        if (generation != null && chunk != null)
            LegacySectionSnapshot.capture(chunk, task, generation, retainProof);
    }

    public static long[] snapshotMetrics() { return LegacySectionSnapshot.metrics(); }
    public static long snapshotPercentile(int percent) { return LegacySectionSnapshot.percentile(percent); }
    public static boolean snapshotProofReady() { return LegacySectionSnapshot.proofReady(); }
    public static boolean injectStaleSnapshotProof() { return LegacySectionSnapshot.rejectStaleProof(); }

    private static synchronized void callbackReturn(Object receiver) throws ReflectiveOperationException {
        if (TASKS.isEmpty()) return;
        if (completionTask == null) {
            completionTask = Members.field(LegacyAccess.type("LEGACY_RENDERER_COMPLETION"),
                    LegacyNames.member("LEGACY_RENDERER_COMPLETION", "f_1403720"));
        }
        normalReturn(8, completionTask.get(receiver), true);
    }

    private static synchronized void ownerEvent(int event) {
        long thread = Thread.currentThread().threadId();
        // Only the client frame/reload can establish ownership, never a worker callback.
        if (owner == 0 && (event == 1 || event == 3)) owner = thread;
        boolean owning = owner == thread;
        long before = Hook.ravelRendererEvent(0, 0, thread, owning);
        Hook.ravelRendererEvent(event, before, thread, owning);
        if (owning && LegacyRuntime.PROOF) COUNTS[event]++;
    }

    private static synchronized void capture(Object task) {
        if (task == null || TASKS.containsKey(task)) return;
        // Missing or overflow-refused tasks are never relabelled at later worker/compile entry.
        if (TASKS.size() >= TASK_LIMIT) return;
        TASKS.put(task, currentGeneration());
        if (LegacyRuntime.PROOF && proofArmed && proofTask == null) proofTask = task;
    }

    private static synchronized void normalReturn(int event, Object task, boolean finished) {
        Long captured = TASKS.get(task);
        if (finished && proofTask != null && task == proofTask) proofTaskFinished = true;
        else if (finished) TASKS.remove(task);
        if (captured == null) return;
        long thread = Thread.currentThread().threadId();
        boolean owning = owner == thread;
        boolean accepted = LegacyRuntime.PROOF && Hook.ravelRendererAccept(captured);
        Hook.ravelRendererEvent(event, captured, thread, owning);
        if (LegacyRuntime.PROOF && accepted) {
            COUNTS[event]++;
            if (!owning) workerObserved = true;
        } else if (LegacyRuntime.PROOF) {
            staleReturns++;
        }
    }

    public static long currentGeneration() {
        return Hook.ravelRendererEvent(0, 0, Thread.currentThread().threadId(), false);
    }

    /** Point-in-time observation only, not a GL lease or permission to cancel vanilla work. */
    public static boolean accepts(long captured) {
        return Hook.ravelRendererAccept(captured);
    }

    public static long ownerThread() { return owner; }
    public static boolean workerObserved() { return workerObserved; }
    public static synchronized long count(int event) { return COUNTS[event]; }

    public static synchronized void armStaleProof() {
        if (!LegacyRuntime.PROOF) throw new IllegalStateException("Renderer proof is disabled");
        if (proofArmed) throw new IllegalStateException("Renderer proof is already armed");
        proofArmed = true;
    }

    public static synchronized boolean staleProofReady() {
        Long captured = TASKS.get(proofTask);
        return proofTask != null && captured != null && Hook.ravelRendererAccept(captured);
    }

    /** Injects only an observer call, never vanilla work or an actual task completion. */
    public static synchronized boolean injectStaleProof() {
        if (!LegacyRuntime.PROOF || proofTask == null) return false;
        try {
            long acceptedBefore = COUNTS[7];
            long rejectedBefore = staleReturns;
            normalReturn(7, proofTask, false);
            return COUNTS[7] == acceptedBefore && staleReturns == rejectedBefore + 1;
        } finally {
            if (proofTaskFinished) TASKS.remove(proofTask);
            proofTask = null;
            proofTaskFinished = false;
            proofArmed = false;
        }
    }
}
