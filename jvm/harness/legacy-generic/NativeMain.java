package dev.ravel.jvm;

import java.io.IOException;
import java.lang.classfile.ClassFile;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Real agent/native checks run in fresh game directories and separate JVMs. */
public final class NativeMain {
    private NativeMain() { }
    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    public static void main(String[] arguments) throws Exception {
        if (arguments[0].equals("prepare")) {
            Path classes = Path.of(arguments[1]);
            entry(classes, GameNames.CLIENT_ENTRY_POINT, true);
            entry(classes, GameNames.SERVER_ENTRY_POINT, true);
            entry(Path.of(arguments[2]), GameNames.SERVER_ENTRY_POINT, false);
            Files.writeString(Path.of(arguments[3]), GameNames.CLIENT_ENTRY_POINT + "\n" + GameNames.SERVER_ENTRY_POINT + "\n");
        } else if (arguments[0].equals("none")) {
            require(Hook.ravelModSurface() == null && Hook.ravelProfilingReport() == null,
                    "premain initialized without a distribution entry point");
            require(Agent.transformedClasses() == 0 && Agent.rewrittenMethods() == 0, "inert launch rewrote targets");
        } else if (arguments[0].equals("missing")) {
            Class.forName(GameNames.SERVER_ENTRY_POINT);
            require(Hook.ravelModSurface() == null, "entry class without main installed a side");
        } else throw new IllegalArgumentException(arguments[0]);
    }
    private static void entry(Path root, String dotted, boolean hasMain) throws IOException {
        ClassDesc owner = ClassDesc.of(dotted);
        byte[] bytes = ClassFile.of().build(owner, builder -> {
            builder.withFlags(ClassFile.ACC_PUBLIC);
            if (hasMain) builder.withMethodBody("main", MethodTypeDesc.ofDescriptor("([Ljava/lang/String;)V"),
                    ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC,
                    code -> code.aload(0).invokestatic(ClassDesc.of("dev.ravel.jvm.NativeMain"), "run",
                            MethodTypeDesc.ofDescriptor("([Ljava/lang/String;)V")).return_());
        });
        Path file = root.resolve(dotted.replace('.', '/') + ".class");
        Files.createDirectories(file.getParent());
        Files.write(file, bytes);
    }
    static List<String> fields(String document) {
        require(document != null, "missing native document");
        List<String> fields = new ArrayList<>();
        for (int at = 0; at < document.length();) {
            int colon = document.indexOf(':', at);
            require(colon > at, "invalid field frame");
            int length = Integer.parseInt(document.substring(at, colon));
            at = colon + 1;
            fields.add(document.substring(at, at + length));
            at += length;
        }
        return fields;
    }
    private static long eventCalls() {
        List<String> report = fields(Hook.ravelProfilingReport());
        require(report.get(1).equals("true"), "profiling must be enabled for dispatch evidence");
        int count = Integer.parseInt(report.get(5));
        long calls = 0;
        for (int index = 0; index < count; index++) calls += Long.parseLong(report.get(6 + index * 7 + 3));
        return calls;
    }
    public static void run(String[] arguments) throws Exception {
        if (arguments[1].equals("initialization-fixture")) {
            Class.forName("dev.ravel.jvm.InitializationMain").getMethod("run", String[].class)
                    .invoke(null, (Object) arguments);
            return;
        }
        String expectedSide = arguments[0];
        List<String> surface = fields(Hook.ravelModSurface());
        require(surface.get(1).equals(expectedSide), "entry point selected wrong side: " + surface);
        require(Integer.parseInt(surface.get(3)) > 0, "initialized set contains no mods");
        String before = Hook.ravelModSurface();
        // A second entrant cannot reinstall with a different side or reset the native set.
        Agent.entered(expectedSide.equals("client") ? 1 : 0, NativeMain.class);
        require(before.equals(Hook.ravelModSurface()), "second entry reinstalled the runtime");
        if (arguments[1].equals("load")) load();
        if (arguments[1].equals("cluster")) cluster(expectedSide);
        if (arguments[1].equals("renderer")) renderer();
        System.out.println("legacy " + arguments[1] + " side=" + expectedSide + " checked");
    }
    private static void renderer() throws Exception {
        LegacyRenderer.enter("legacyRendererFrame", null);
        long initial = LegacyRenderer.currentGeneration();
        require(initial > 0 && LegacyRenderer.accepts(initial), "no renderer generation");
        captureAdmission(initial);
        snapshot(initial);
        LegacyRenderer.armStaleProof();
        Object queued = constructedRendererTask();
        require(LegacyRenderer.staleProofReady(), "transformed constructor did not capture initialized receiver");
        long compiled = LegacyRenderer.count(7);
        Thread worker = new Thread(() -> {
            try {
                LegacyRenderer.exit("legacyRendererCompile", null, queued);
            } catch (ReflectiveOperationException failure) {
                throw new AssertionError(failure);
            }
        });
        worker.start();
        worker.join();
        require(LegacyRenderer.count(7) == compiled + 1 && LegacyRenderer.workerObserved(),
                "worker compile normal return did not preserve task generation");
        LegacyRenderer.enter("legacyResourceReload", null);
        long reloaded = LegacyRenderer.currentGeneration();
        require(reloaded > initial && !LegacyRenderer.accepts(initial), "reload retained old task token");
        require(Hook.ravelRendererSnapshot(initial, LegacyMeshWork.probeKey(), new int[5832], new byte[5832],
                new byte[5832]).length == 0, "retired snapshot batch accepted");
        require(LegacyRenderer.injectStaleProof(), "injected stale task observation was not rejected");
        LegacyRenderer.enter("legacyRendererWorker", queued);
        LegacyRenderer.exit("legacyRendererCompile", null, queued);
        require(LegacyRenderer.count(7) == compiled + 1, "queued old task relabelled on worker entry");
        LegacyRenderer.exit("legacyRendererCompile", null, new Object());
        require(LegacyRenderer.count(7) == compiled + 1, "untracked task acquired current token");
        LegacyRenderer.enter("legacyRendererUnload", new Object());
        long replaced = LegacyRenderer.currentGeneration();
        require(replaced > reloaded && !LegacyRenderer.accepts(reloaded),
                "nonnull world replacement retained old task token");
        LegacyRenderer.enter("legacyRendererUnload", null);
        require(LegacyRenderer.currentGeneration() > replaced && !LegacyRenderer.accepts(replaced),
                "unload retained old task token");
        Object[] retained = new Object[4097];
        for (int index = 0; index < retained.length; index++) {
            retained[index] = new Object();
            LegacyRenderer.exit("legacyRendererTaskCreate", retained[index], null);
        }
        LegacyRenderer.exit("legacyRendererCompile", null, retained[retained.length - 1]);
        require(LegacyRenderer.count(7) == compiled + 1, "bounded overflow relabelled untracked task");
        java.lang.ref.Reference.reachabilityFence(retained);
        meshWork();
    }

    private static void snapshot(long generation) {
        int[] states = new int[5832];
        byte[] light = new byte[5832];
        byte[] geometry = new byte[5832];
        for (int i = 0; i < states.length; i++) {
            states[i] = ((i * 179) & 1048575) << 4 | (i & 15);
            light[i] = (byte) i;
        }
        states[0] = 0;
        states[5831] = 0xffffff;
        for (int i = 0; i < states.length; i++) geometry[i] = (byte) ((states[i] >>> 4) & 1);
        long key = LegacyMeshWork.reserve(generation);
        long[] receipt = Hook.ravelRendererSnapshot(generation, key, states, light, geometry);
        LegacyMeshWork.commit(key, receipt, 1);
        require(receipt.length == 8 && receipt[0] == 1 && receipt[1] > 0
                && receipt[2] == 0 && receipt[3] == 5832 && receipt[4] == 34992
                && receipt[5] > 34992 && receipt[6] == 1 && receipt[7] == key,
                "native dense state/light/geometry batch roundtrip failed");
        snapshotFailures(generation, states, light, geometry);
    }

    // Each failure settles a really reserved key: it must be released and never counted as admitted.
    private static void failure(long generation, int[] states, byte[] light, byte[] geometry,
            long code, long a, long b, long c) {
        long key = LegacyMeshWork.reserve(generation);
        int held = LegacyMeshWork.occupied();
        long[] receipt = Hook.ravelRendererSnapshot(generation, key, states, light, geometry);
        require(key > 0 && receipt.length == 8 && receipt[0] == -1 && receipt[1] == code && receipt[2] == a
                && receipt[3] == b && receipt[4] == c && receipt[5] == 0 && receipt[6] == 0 && receipt[7] == key,
                "native capture failure " + code + " not surfaced: " + java.util.Arrays.toString(receipt));
        long[] before = LegacyMeshWork.metrics();
        LegacyMeshWork.commit(key, receipt, 1);
        long[] after = LegacyMeshWork.metrics();
        require(after[0] == before[0] && after[1] == before[1] && after[43] == before[43] + 1
                && after[44] == code && after[45] == a && LegacyMeshWork.occupied() == held - 1,
                "Java counted a capture failure as admitted or kept its key");
    }

    private static void snapshotFailures(long generation, int[] states, byte[] light, byte[] geometry) {
        long probe = LegacyMeshWork.probeKey();
        require(Hook.ravelRendererSnapshot(0, probe, states, light, geometry).length == 0
                && Hook.ravelRendererSnapshot(generation, 0, states, light, geometry).length == 0
                && Hook.ravelRendererSnapshot(generation, -1, states, light, geometry).length == 0,
                "stale generation or reserved key reached capture validation");
        failure(generation, null, light, null, 7, 5, 0, 0);
        failure(generation, states, null, geometry, 7, 2, 0, 0);
        failure(generation, new int[5831], light, geometry, 2, 5831, 5832, 5832);
        failure(generation, states, new byte[5833], geometry, 2, 5832, 5833, 5832);
        failure(generation, states, light, new byte[5831], 2, 5832, 5832, 5831);
        geometry[1] = 2;
        failure(generation, states, light, geometry, 4, 1, 2, 0);
        geometry[1] = (byte) ((states[1] >>> 4) & 1);
        states[2] = states[1];
        geometry[2] = (byte) (geometry[1] ^ 1);
        failure(generation, states, light, geometry, 5, states[1], 0, 0);
        geometry[2] = geometry[1];
        for (int invalid : new int[] {-1, 0x1000000}) {
            states[0] = invalid;
            failure(generation, states, light, geometry, 3, 0, invalid, 0);
        }
        long key = LegacyMeshWork.reserve(generation);
        int held = LegacyMeshWork.occupied();
        try {
            LegacyMeshWork.commit(key, new long[] {1, 0, 0, 5832, 34992, 1, 1, key + 1}, 1);
            throw new AssertionError("receipt for another key was accepted");
        } catch (IllegalStateException expected) {
            require(LegacyMeshWork.occupied() == held - 1, "malformed receipt leaked its Java key");
        }
    }

    private static final int STONE = 1 << 4;
    private static final int NON_CUBE = 20 << 4;

    // Deterministic geometry section: one full-opaque cube, one non-cube state and optionally a sealed layer.
    private static long submit(long generation, boolean sealed) {
        int[] states = new int[5832];
        byte[] geometry = new byte[5832];
        states[(1 * 18 + 1) * 18 + 1] = STONE;
        states[(1 * 18 + 1) * 18 + 3] = NON_CUBE;
        if (sealed) for (int z = 0; z < 18; z++) for (int x = 0; x < 18; x++) states[(9 * 18 + z) * 18 + x] = STONE;
        for (int i = 0; i < states.length; i++) geometry[i] = (byte) (states[i] == STONE ? 1 : 0);
        long key = LegacyMeshWork.reserve(generation);
        require(key > 0, "Java key table refused deterministic geometry job");
        long[] receipt = Hook.ravelRendererSnapshot(generation, key, states, new byte[5832], geometry);
        LegacyMeshWork.commit(key, receipt, 1);
        require(receipt.length == 8 && receipt[2] == 0 && receipt[7] == key && (receipt[6] & ~1L) == 0,
                "current-generation geometry batch did not return an admission receipt");
        return receipt[6];
    }

    private static void drain(long target) throws InterruptedException {
        long deadline = System.nanoTime() + 30_000_000_000L;
        while (LegacyMeshWork.metrics()[3] < target) {
            require(System.nanoTime() < deadline, "native geometry completions were not drained");
            LegacyRenderer.enter("legacyRendererUpdate", null);
            Thread.sleep(1);
        }
    }

    // Drained before any reload: one exposed cube plus one unsupported state, then a sealed full layer.
    private static long[] geometryRecords(long current) throws InterruptedException {
        long[] before = LegacyMeshWork.metrics();
        require(submit(current, false) == 1, "current geometry job was not admitted");
        drain(before[3] + 1);
        long[] one = LegacyMeshWork.metrics();
        require(one[5] == before[5] + 1 && one[7] == before[7] + 1 && one[9] - before[9] == 24
                && one[10] - before[10] == 36 && one[16] == before[16] && one[17] == 0,
                "full-cube/unsupported geometry metadata was not drained exactly");
        require(submit(current, true) == 1, "sealed geometry job was not admitted");
        drain(one[3] + 1);
        long[] sealed = LegacyMeshWork.metrics();
        require(sealed[5] == one[5] + 1 && sealed[16] == one[16] + 1 && sealed[9] > one[9] + 24
                && sealed[17] == 0, "sealed full-cube layer did not split section visibility");
        return sealed;
    }

    // Deterministic native proof, not live evidence: admitted jobs are retired before Java drains them.
    private static void meshWork() throws Exception {
        LegacyRenderer.enter("legacyResourceReload", null);
        long current = LegacyRenderer.currentGeneration();
        long[] drained = geometryRecords(current);
        int admitted = 0;
        for (int index = 0; index < 12; index++) admitted += (int) submit(current, false);
        long[] stats = Hook.ravelRendererWork(1, current, 0);
        long[] full = LegacyMeshWork.metrics();
        require(stats.length == 18 && stats[1] == current && stats[15] == 8 && stats[17] == 0
                && admitted == stats[15] && stats[3] == stats[15] && stats[8] == stats[15]
                && stats[7] == stats[16] && stats[9] == stats[16] && full[1] - drained[1] == 12 - admitted
                && full[0] - drained[0] == admitted, "native work did not admit exactly its job and byte ceilings");
        LegacyRenderer.enter("legacyResourceReload", null);
        long next = LegacyRenderer.currentGeneration();
        long[] after = LegacyMeshWork.metrics();
        require(after[18] - full[18] == admitted && after[30] == full[30] + 1 && after[31] == full[31]
                && after[51] == current && after[49] == full[49] && LegacyMeshWork.occupied() == 0,
                "reload did not retire undrained keys and prove the retired pool drained");
        long[] retired = Hook.ravelRendererWork(3, next, 0);
        require(retired.length == 18 && retired[1] == current && retired[2] == 1 && retired[14] == 0
                && java.util.Arrays.stream(retired, 3, 8).allMatch(v -> v == 0) && retired[8] == 8
                && retired[9] == retired[16], "retired pool final stats lost their actual high-water marks");
        require(Hook.ravelRendererWork(0, current, 0).length == 0
                && Hook.ravelRendererWork(1, current, 0).length == 0
                && Hook.ravelRendererWork(3, current, 0).length == 0
                && LegacyMeshWork.injectStaleProof(current)
                && LegacyMeshWork.metrics()[42] == after[42] + 1 && LegacyMeshWork.occupied() == 0,
                "retired generation work was published");
        restartAndClose(next, after);
    }

    private static void restartAndClose(long next, long[] after) throws InterruptedException {
        require(submit(next, false) == 1, "work did not restart for the subsequent generation");
        drain(after[3] + 1);
        require(LegacyMeshWork.metrics()[3] == after[3] + 1 && LegacyMeshWork.metrics()[17] == 0,
                "restarted generation published a retired key");
        drainedBeforeReceipt(next);
        require(submit(next, false) == 1, "second restarted job was not admitted");
        long admittedKey = LegacyMeshWork.reserve(next);
        long[] admittedReceipt = capturedReceipt(next, admittedKey);
        long pending = LegacyMeshWork.reserve(next);
        require(pending > 0, "Java key table refused pending close reservation");
        long[] before = LegacyMeshWork.metrics();
        Thread late = lateReceipt(admittedKey, admittedReceipt, pending);
        long[] closed = LegacyMeshWork.shutdown();
        long[] settled = LegacyMeshWork.metrics();
        require(LegacyMeshWork.occupied() == 0 && settled[50] == before[50] + 1
                && settled[0] == before[0] + 1 && settled[18] == before[18] + 2,
                "shutdown returned before admitted and unadmitted late receipts settled");
        late.join();
        require(closed.length == 18 && closed[1] == next && closed[2] == 1 && closed[17] == 0
                && java.util.Arrays.stream(closed, 3, 8).allMatch(v -> v == 0)
                && closed[14] == 0 && LegacyMeshWork.occupied() == 0,
                "shutdown did not drain every native reservation and withdraw every Java key");
        require(LegacyMeshWork.reserve(next) == 0 && LegacyMeshWork.occupied() == 0,
                "closed generation reserved a Java key");
        require(Hook.ravelRendererSnapshot(next, LegacyMeshWork.probeKey(), new int[5832], new byte[5832],
                new byte[5832]).length == 0, "closed generation admitted new work");
    }

    private static long[] capturedReceipt(long generation, long key) {
        require(key > 0, "Java key table refused delayed receipt reservation");
        long[] receipt = Hook.ravelRendererSnapshot(generation, key, new int[5832], new byte[5832],
                new byte[5832]);
        require(receipt.length == 8 && receipt[0] == 1 && receipt[2] == 0
                && receipt[6] == 1 && receipt[7] == key, "delayed JNI capture was not admitted");
        return receipt;
    }

    // A native completion can drain while Java still owns the uncommitted admitted receipt.
    private static void drainedBeforeReceipt(long generation) throws InterruptedException {
        long[] before = LegacyMeshWork.metrics();
        long key = LegacyMeshWork.reserve(generation);
        long[] receipt = capturedReceipt(generation, key);
        drain(before[3] + 1);
        require(LegacyMeshWork.occupied() == 1 && LegacyMeshWork.metrics()[0] == before[0],
                "draining an uncommitted receipt lost its reserved slot");
        LegacyMeshWork.commit(key, receipt, 1);
        long[] settled = LegacyMeshWork.metrics();
        require(LegacyMeshWork.occupied() == 0 && settled[0] == before[0] + 1
                && settled[18] == before[18] && settled[50] == before[50],
                "drained-before-commit admission was lost or counted as retired");
    }

    // Settle real JNI admission only after shutdown retires both pending slots, never on a timer.
    private static Thread lateReceipt(long admittedKey, long[] receipt, long pending) {
        Thread late = new Thread(() -> {
            long deadline = System.nanoTime() + 30_000_000_000L;
            while (!LegacyMeshWork.retiredPending(admittedKey) || !LegacyMeshWork.retiredPending(pending)) {
                require(System.nanoTime() < deadline, "shutdown never retired pending receipt slots");
                try {
                    Thread.sleep(1);
                } catch (InterruptedException interrupted) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException("Interrupted awaiting pending receipt retirement", interrupted);
                }
            }
            LegacyMeshWork.commit(admittedKey, receipt, 1);
            LegacyMeshWork.commit(pending, new long[0], 1);
        });
        late.start();
        return late;
    }

    // Pure admission checks intentionally do not stand in for live game-region evidence.
    private static void captureAdmission(long generation) throws Exception {
        long[] before = LegacyRenderer.snapshotMetrics();
        LegacySectionSnapshot.capture(null, null, 0, false);
        require(LegacyRenderer.snapshotMetrics()[7] == before[7] + 1,
                "stale capture reached game access");
        java.lang.reflect.Field field = LegacySectionSnapshot.class.getDeclaredField("BUSY");
        field.setAccessible(true);
        java.util.concurrent.atomic.AtomicBoolean busy =
                (java.util.concurrent.atomic.AtomicBoolean) field.get(null);
        require(busy.compareAndSet(false, true), "fixture unexpectedly entered busy capture");
        try {
            LegacySectionSnapshot.capture(null, null, generation, false);
            require(LegacyRenderer.snapshotMetrics()[8] == before[8] + 1,
                    "busy capture reached game access instead of skipping");
        } finally { busy.set(false); }
    }

    private static Object constructedRendererTask() throws Exception {
        HookTarget mapped = java.util.Arrays.stream(Hook.TARGETS)
                .filter(hook -> hook.hookMethod().equals("legacyRendererTaskCreate")).findFirst().orElseThrow();
        HookTarget fixture = new HookTarget("fixture/TrackedRendererTask", "<init>",
                "(Ljava/lang/Object;Ljava/lang/Object;D)V", false,
                mapped.hookMethod(), mapped.hookDescriptor());
        byte[] original = GenericMain.target(fixture);
        byte[] transformed = new Transformer(new HookTarget[] {fixture}, true).transform(
                null, NativeMain.class.getClassLoader(), fixture.internalClass(), null, null, original);
        require(transformed != null, "task constructor fixture was not transformed");
        class Child extends ClassLoader {
            Child() { super(NativeMain.class.getClassLoader()); }
            Class<?> define() { return defineClass(null, transformed, 0, transformed.length); }
        }
        return new Child().define().getConstructor(Object.class, Object.class, double.class)
                .newInstance(new Object(), new Object(), 0.0);
    }


    private static void load() throws ReflectiveOperationException {
        HookTarget observation = java.util.Arrays.stream(Hook.TARGETS)
                .filter(target -> target.hookDescriptor().equals("()V")).findFirst().orElseThrow();
        byte[] bytes = ClassFile.of().build(ClassDesc.of("fixture.SharedHookCrossing"), builder -> {
            builder.withFlags(ClassFile.ACC_PUBLIC);
            builder.withMethodBody("cross", MethodTypeDesc.ofDescriptor("()J"),
                    ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, code -> code
                            .invokestatic(ClassDesc.of("dev.ravel.jvm.Hook"), observation.hookMethod(), MethodTypeDesc.ofDescriptor("()V"))
                            .invokestatic(ClassDesc.of("dev.ravel.jvm.Hook"), "nativeInvocations", MethodTypeDesc.ofDescriptor("()J"))
                            .lreturn());
        });
        class Child extends ClassLoader {
            Child() { super(NativeMain.class.getClassLoader()); }
            Class<?> define() { return defineClass(null, bytes, 0, bytes.length); }
        }
        Child child = new Child();
        require(child.loadClass("dev.ravel.jvm.Hook") == Hook.class
                && child.loadClass("dev.ravel.jvm.Agent") == Agent.class, "split bootstrap class identity");
        long before = Hook.nativeInvocations();
        Class<?> crossing = child.define();
        require(crossing.getClassLoader() == child, "crossing did not use child game loader");
        require(((Long) crossing.getMethod("cross").invoke(null)) == before + 1
                && Hook.nativeInvocations() == before + 1, "child JNI crossing did not reach shared native counter");
    }
    private static void cluster(String side) {
        long before = eventCalls();
        Hook.ravelLegacyEvent(side.equals("client") ? "client-tick" : "server-tick");
        long tick = eventCalls();
        require(tick > before, "tick did not dispatch a native mod callback");
        Hook.ravelLegacyEvent("resources-reloaded");
        long reload = eventCalls();
        require(reload > tick, "reload did not dispatch a native mod callback");
        try {
            Hook.ravelLegacyEvent("not-a-reviewed-event");
            throw new AssertionError("unknown semantic event accepted");
        } catch (RuntimeException expected) {
            require(eventCalls() == reload, "invalid semantic event delivered callbacks");
        }
        if (side.equals("dedicated-server")) {
            Hook.ravelLegacyEvent("client-tick");
            require(eventCalls() == reload, "dedicated server delivered client-only event");
        }
    }
}
