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
