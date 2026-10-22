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
        LegacyGeometryProof.selected(); // Validate explicit selection; default performs no GL work.
        switch (id) {
            case "legacyResourceReload" -> lifecycle(1);
            case "legacyAtlasReload" -> {
                if (LegacyGeometryProof.selected()) lifecycle(1);
            }
            case "legacyRendererUnload", "legacyRendererClose" -> lifecycle(2);
            case "legacyRendererFrame" -> {
                ownerEvent(3);
                if (LegacyGeometryProof.selected() && LegacyWorldDraw.distanceChanged()) lifecycle(1);
                LegacyMeshWork.frame();
                if (LegacyGeometryProof.selected()) { LegacyBakedCapture.frame(); LegacyWorldDraw.frame(); }
            }
            case "legacyRendererTerrain" -> ownerEvent(4);
            case "legacyRendererLayer" -> {
                ownerEvent(5);
                LegacyBakedCapture.draw();
                LegacyWorldDraw.layer(first);
            }
            case "legacyRendererUpdate" -> {
                ownerEvent(6);
                LegacyMeshWork.poll();
            }
            case "legacyRendererUpload" -> ownerEvent(9);
            default -> { }
        }
    }

    // Native invalidation joins its workers before returning; Java then forgets every retired key and
    // checks the retired pool's final stats against the generation observed before the event.
    private static void lifecycle(int event) {
        long retiring = currentGeneration();
        ownerEvent(event);
        LegacyBakedCapture.clear();
        try {
            LegacyGeometryProof.retire(retiring);
        } finally {
            try {
                LegacyVanillaGeometry.clear(retiring);
            } finally {
                try { LegacyMeshWork.retire(retiring); }
                finally { LegacyGeometryProof.flushMetrics(); }
            }
        }
    }

    // These returns can include aborted compilation or a cancelled callback's early return.
    // Generation admission is not a compilation/publication outcome claim.
    static void exit(String id, Object receiver, Object first) throws ReflectiveOperationException {
        switch (id) {
            case "legacyRendererLayer" -> LegacyWorldDraw.layer(null);
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
        LegacyBakedCapture.finish(task);
        if (generation != null && chunk != null)
            LegacySectionSnapshot.capture(chunk, task, generation, retainProof);
    }

    public static long[] snapshotMetrics() { return LegacySectionSnapshot.metrics(); }
    public static long[] bakedMetrics() { return LegacyBakedCapture.metrics(); }
    public static long[][] bakedGoldens() { return LegacyBakedCapture.goldens(); }
    public static long[] bakedWorld() { return LegacyBakedCapture.world(); }
    public static byte[][] bakedReadbacks(int scene) { return LegacyBakedCapture.readbacks(scene); }
    public static void bakedDesignate(int scene, int x, int y, int z) { LegacyBakedCapture.designate(scene, x, y, z); }
    public static void bakedPhase(int phase) { LegacyBakedCapture.phase(phase); }
    public static int bakedCoverage(int phase) { return LegacyBakedCapture.coverage(phase); }
    public static byte[] bakedComposite(Runnable render, int width, int height, int expected, boolean finish) {
        return LegacyBakedCapture.composite(render, width, height, expected, finish);
    }
    public static int bakedCompositeDesignated() { return LegacyBakedCapture.compositeDesignated(); }
    public static boolean skipCompositeUpdate() { return LegacyCompositeCapture.replaying(); }
    public static boolean productionReady(int x, int y, int z) { return LegacyWorldDraw.ready(x, y, z); }
    public static long[] productionBudget() { return Hook.ravelRendererBakedLease(0, 0, 5); }
    public static long[] productionDedupe() { return Hook.ravelRendererBakedLease(0, 0, 6); }
    public static long[] vanillaBufferBytes() { return LegacyVanillaBuffers.metrics(); }
    public static void vanillaBufferDeleted(Object buffer) { LegacyVanillaBuffers.deleted(buffer); }
    public static long translucentAdmissions() { return LegacyWorldDraw.translucentAdmissions(); }
    public static long[] productionSorts() { return LegacyWorldResort.metrics(); }
    public static long[] productionSectionSorts(int x, int y, int z) { return LegacyWorldDraw.sorts(x, y, z); }
    public static float[] productionSectionSortCamera(int x, int y, int z) {
        return LegacyWorldDraw.sortCamera(x, y, z);
    }
    public static void productionWarm(int x, int z) throws ReflectiveOperationException { LegacyWorldDraw.warm(x, z); }
    public static boolean productionNegative(int x, int y, int z, int kind) {
        return LegacyWorldDraw.negative(x, y, z, kind);
    }
    public static long snapshotPercentile(int percent) { return LegacySectionSnapshot.percentile(percent); }
    public static boolean snapshotProofReady() { return LegacySectionSnapshot.proofReady(); }
    public static boolean injectStaleSnapshotProof() { return LegacySectionSnapshot.rejectStaleProof(); }

    public static long[] meshMetrics() { return LegacyMeshWork.metrics(); }
    public static String[] meshMetricNames() { return LegacyMeshWork.NAMES.clone(); }
    public static String[] meshSeries() { return LegacyMeshWork.SERIES.clone(); }
    public static long meshPercentile(int series, int percent) { return LegacyMeshWork.percentile(series, percent); }
    public static void beginMeshStress() { LegacyMeshWork.beginStress(); }
    public static void endMeshStress() { LegacyMeshWork.endStress(); }
    public static boolean injectStaleMeshProof(long retired) { return LegacyMeshWork.injectStaleProof(retired); }
    public static int meshKeysHeld() { return LegacyMeshWork.occupied(); }
    public static long[] shutdownMeshWork() {
        long retiring = currentGeneration();
        LegacyBakedCapture.clear();
        long[] result;
        try {
            LegacyGeometryProof.retire(retiring);
        } finally {
            try {
                LegacyVanillaGeometry.clear(retiring);
            } finally {
                try { result = LegacyMeshWork.shutdown(); }
                finally { LegacyGeometryProof.flushMetrics(); }
            }
        }
        return result;
    }

    public static boolean geometryProofSelected() { return LegacyGeometryProof.selected(); }
    public static long[] geometryProofMetrics() { return LegacyGeometryProof.metrics(); }
    public static long[] shadedNativeResidency() { return LegacyGeometryProof.shadedNativeResidency(); }
    public static long[] shadedGlResidency() { return LegacyGeometryProof.shadedGlResidency(); }
    public static long[] productionWorldDraw() { return LegacyGeometryProof.worldMetrics(); }
    public static String[] geometryProofMetricNames() { return LegacyGeometryProof.NAMES.clone(); }
    public static String[] geometryProofInfo() { return LegacyGeometryProof.info(); }
    public static long[] geometryProofIdentity() { return LegacyGeometryProof.identity(); }
    public static byte[] geometryProofPixels() { return LegacyGeometryProof.pixels(false); }
    public static byte[] geometryProofReferencePixels() { return LegacyGeometryProof.pixels(true); }
    public static float[] geometryProofDepths() { return LegacyGeometryProof.depths(); }
    public static long[] geometryProofCompareVanilla(Runnable draw) { return LegacyGeometryProof.compareVanilla(draw); }
    public static long[] geometryProofComparePublishedVanilla() throws ReflectiveOperationException {
        return LegacyVanillaGeometry.compareResident();
    }
    public static byte[] geometryProofVanillaPixels() { return LegacyGeometryProof.vanillaPixels(); }
    public static void geometryProofBindVanillaBuffer(int buffer) { LegacyGeometryProof.bindVanillaBuffer(buffer); }

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

    static synchronized long taskGeneration(Object task) {
        Long captured = TASKS.get(task);
        return captured == null ? 0 : captured;
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
