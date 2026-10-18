package dev.ravel.metrics;

import java.io.BufferedWriter;
import java.io.IOException;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.management.ClassLoadingMXBean;
import java.lang.management.ManagementFactory;
import java.lang.management.MemoryUsage;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
import java.security.ProtectionDomain;
import java.util.concurrent.ConcurrentHashMap;
import java.util.Set;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Label;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

/** Java 8 observer shared by Forge and official 1.12.2 clients; measurements remain local. */
public final class BenchmarkAgent {
    private static final long SECOND = 1_000_000_000L;
    private static final long SEED = 8675309L;
    private static final String CLASS_NAME = "dev.ravel.metrics.BenchmarkAgent";
    private static final String OWNER = "dev/ravel/metrics/BenchmarkAgent";
    private static final ThreadLocal<Timing> FRAME = ThreadLocal.withInitial(Timing::new);
    private static final Set<String> MEASURED = new java.util.HashSet<>(java.util.Arrays.asList(
        "spawn_idle", "square_exploration", "loaded_chunk_stress"));
    private static final ThreadLocal<Timing> TICK = ThreadLocal.withInitial(Timing::new);
    private static final ThreadLocal<Timing> COMPILE = ThreadLocal.withInitial(Timing::new);
    private static final ThreadLocal<Timing> UPLOAD = ThreadLocal.withInitial(Timing::new);
    private static final Set<String> rendererObserved = ConcurrentHashMap.newKeySet();
    private static BufferedWriter chunks;
    private static BufferedWriter frames;
    private static BufferedWriter ticks;
    private static BufferedWriter memory;
    private static BufferedWriter events;
    private static BufferedWriter trajectory;
    private static Path outputDirectory;
    private static volatile boolean shutdownRequested;
    private static volatile boolean finished;
    private static volatile boolean clientObserved;
    private static volatile boolean serverObserved;
    private static volatile String scene = "startup";
    private static long started;
    private static long menuAt;
    private static long creationAt;
    private static long sceneAt;
    private static long previousFrameExit;
    private static String previousFrameScene = "startup";
    private static Object client;
    private static Object lastScreen;
    private static ClientAccess access;
    private static PlayerAccess playerAccess;
    private static double originX;
    private static double originZ;

    private BenchmarkAgent() { }

    public static void premain(String argument, Instrumentation instrumentation) {
        started = System.nanoTime();
        sceneAt = started;
        try {
            if (argument == null || argument.trim().isEmpty()) {
                throw new IllegalArgumentException("javaagent requires an output directory");
            }
            outputDirectory = Paths.get(argument).toAbsolutePath();
            Path directory = outputDirectory;
            Files.createDirectories(directory);
            events = csv(directory, "events.csv", "event,monotonic_ns,detail");
            frames = csv(directory, "frames.csv", "scene,monotonic_ns,frame_ns");
            ticks = csv(directory, "ticks.csv", "scene,monotonic_ns,tick_ns");
            memory = csv(directory, "jvm.csv",
                "scene,monotonic_ns,heap_used_bytes,heap_committed_bytes,heap_max_bytes,loaded_classes,total_loaded_classes,unloaded_classes,nonheap_used_bytes,direct_used_bytes,mapped_used_bytes");
            trajectory = csv(directory, "trajectory.csv", "scene,monotonic_ns,scene_ns,x,y,z,yaw,pitch");
            chunks = csv(directory, "chunks.csv", "scene,monotonic_ns,kind,duration_ns");
            marker("agent_started", started, "seed=" + SEED);
            instrumentation.addTransformer(new ObserverTransformer());
            Runtime.getRuntime().addShutdownHook(new Thread(BenchmarkAgent::onShutdown, "benchmark-shutdown"));
            Thread sampler = new Thread(BenchmarkAgent::sampleLoop, "benchmark-jvm-sampler");
            sampler.setDaemon(true);
            sampler.start();
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    private static BufferedWriter csv(Path directory, String name, String header) throws IOException {
        BufferedWriter writer = Files.newBufferedWriter(directory.resolve(name), StandardCharsets.UTF_8,
            StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
        writer.write(header);
        writer.newLine();
        writer.flush();
        return writer;
    }

    public static void frameEnter(Object minecraft) {
        try {
            Timing timing = FRAME.get();
            if (timing.depth++ == 0) {
                timing.start = System.nanoTime();
                if (!clientObserved) {
                    client = minecraft;
                    access = new ClientAccess(minecraft.getClass());
                    clientObserved = true;
                    marker("client_hook_observed", timing.start, minecraft.getClass().getName());
                }
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    public static void frameExit() {
        long now = System.nanoTime();
        try {
            Timing timing = FRAME.get();
            if (--timing.depth != 0 || finished) {
                return;
            }
            if (previousFrameExit != 0L) {
                synchronized (frames) {
                    frames.write(previousFrameScene + "," + now + "," + (now - previousFrameExit));
                    frames.newLine();
                }
            }
            previousFrameExit = now;
            advance(client, now);
            previousFrameScene = scene;
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    public static void tickEnter(Object server) {
        try {
            Timing timing = TICK.get();
            if (timing.depth++ == 0) {
                timing.start = System.nanoTime();
                if (!serverObserved) {
                    serverObserved = true;
                    marker("server_hook_observed", timing.start, server.getClass().getName());
                }
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    public static void tickExit() {
        long now = System.nanoTime();
        try {
            Timing timing = TICK.get();
            if (--timing.depth == 0 && !finished) {
                synchronized (ticks) {
                    ticks.write(scene + "," + now + "," + (now - timing.start));
                    ticks.newLine();
                }
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }
    public static void compileEnter(Object chunk) { rendererEnter(COMPILE, "compile"); }
    public static void compileExit() { rendererExit(COMPILE, "compile"); }
    public static void uploadEnter(Object dispatcher) { rendererEnter(UPLOAD, "upload"); }
    public static void uploadExit() { rendererExit(UPLOAD, "upload"); }

    private static void rendererEnter(ThreadLocal<Timing> local, String kind) {
        try {
            Timing timing = local.get();
            if (timing.depth++ == 0) {
                timing.start = System.nanoTime();
                if (rendererObserved.add(kind)) {
                    marker("renderer_probe_hook", timing.start, kind);
                }
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    private static void rendererExit(ThreadLocal<Timing> local, String kind) {
        long now = System.nanoTime();
        try {
            Timing timing = local.get();
            if (--timing.depth == 0 && !finished) {
                synchronized (chunks) {
                    chunks.write(scene + "," + now + "," + kind + "," + (now - timing.start));
                    chunks.newLine();
                }
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    private static void advance(Object minecraft, long now) throws Throwable {
        Object screen = access.screen.get(minecraft);
        if (screen != lastScreen) {
            lastScreen = screen;
            marker("screen", now, screen == null ? "none" : screen.getClass().getName());
        }
        if (MEASURED.contains(scene) && access.paused.getBoolean(minecraft)) {
            throw new IllegalStateException("Game paused during " + scene + " by screen "
                + (screen == null ? "none" : screen.getClass().getName()));
        }
        if ("startup".equals(scene)) {
            if (screen != null && (access.mainMenu.isInstance(screen)
                    || screen.getClass().getName().equals("lumien.custommainmenu.gui.GuiCustom"))) {
                menuAt = now;
                marker("menu_ready", now, "startup_ns=" + (now - started));
                transition("menu_wait", now);
            }
        } else if ("menu_wait".equals(scene) && now - menuAt >= 5 * SECOND) {
            creationAt = now;
            transition("world_creation", now);
            createWorld(minecraft);
        } else if ("world_creation".equals(scene)) {
            Object player = access.player.get(minecraft);
            if (access.world.get(minecraft) != null && player != null) {
                playerAccess = new PlayerAccess(player);
                Object server = method(minecraft.getClass(), "getIntegratedServer", "func_71401_C").invoke(minecraft);
                Object world = method(server.getClass(), "getWorld", "func_71218_a", int.class).invoke(server, 0);
                Object spawn = method(world.getClass(), "getSpawnPoint", "func_175694_M").invoke(world);
                originX = ((Number) method(spawn.getClass(), "getX", "func_177958_n").invoke(spawn)).doubleValue() + 0.5;
                originZ = ((Number) method(spawn.getClass(), "getZ", "func_177952_p").invoke(spawn)).doubleValue() + 0.5;
                preparePlayer(player);
                marker("world_ready", now, "world_creation_ns=" + (now - creationAt));
                transition("settle", now);
            }
        } else if ("settle".equals(scene) && screen != null && access.paused.getBoolean(minecraft)) {
            access.display.invoke(minecraft, (Object) null);
            marker("screen_dismissed", now, screen.getClass().getName());
        } else if ("settle".equals(scene) && now - sceneAt >= 30 * SECOND) {
            if (!serverObserved) {
                throw new IllegalStateException("Integrated server tick hook was not observed");
            }
            transition("spawn_idle", now);
        } else if ("spawn_idle".equals(scene) && now - sceneAt >= 60 * SECOND) {
            transition("square_exploration", now);
        } else if ("square_exploration".equals(scene) && now - sceneAt >= 120 * SECOND) {
            transition("loaded_chunk_stress", now);
        } else if ("loaded_chunk_stress".equals(scene) && now - sceneAt >= 120 * SECOND) {
            complete(minecraft, now);
        }
        if (playerAccess != null && !finished) {
            movePlayer(now);
        }
    }

    private static void createWorld(Object minecraft) throws ReflectiveOperationException {
        ClassLoader loader = minecraft.getClass().getClassLoader();
        Class<?> settingsClass = gameClass(loader, "net.minecraft.world.WorldSettings", "amx");
        Class<?> gameTypeClass = gameClass(loader, "net.minecraft.world.GameType", "ams");
        Class<?> worldTypeClass = gameClass(loader, "net.minecraft.world.WorldType", "amz");
        Object creative = method(gameTypeClass, "getByID", "func_77146_a", int.class).invoke(null, 1);
        Object defaultType = field(worldTypeClass, "DEFAULT", "field_77137_b").get(null);
        Constructor<?> constructor = settingsClass.getConstructor(long.class, gameTypeClass,
            boolean.class, boolean.class, worldTypeClass);
        Object settings = constructor.newInstance(SEED, creative, true, false, defaultType);
        method(settingsClass, "enableCommands", "func_77166_b").invoke(settings);
        Method launch = method(minecraft.getClass(), "launchIntegratedServer", "func_71371_a",
            String.class, String.class, settingsClass);
        launch.invoke(minecraft, "RavelBenchmark", "RavelBenchmark", settings);
    }

    private static void preparePlayer(Object player) throws Throwable {
        Object settings = field(client.getClass(), "gameSettings", "field_71474_y").get(client);
        field(settings.getClass(), "pauseOnLostFocus", "field_82881_y").setBoolean(settings, false);
        Object capabilities = playerAccess.capabilities.get(player);
        field(capabilities.getClass(), "allowFlying", "field_75101_c").setBoolean(capabilities, true);
        field(capabilities.getClass(), "isFlying", "field_75100_b").setBoolean(capabilities, true);
        method(player.getClass(), "sendPlayerAbilities", "func_71016_p").invoke(player);
        Method chat = method(player.getClass(), "sendChatMessage", "func_71165_d", String.class);
        chat.invoke(player, "/gamerule doDaylightCycle false");
        chat.invoke(player, "/gamerule doWeatherCycle false");
        chat.invoke(player, "/time set 6000");
        chat.invoke(player, "/weather clear 1000000");
        marker("world_controls_sent", System.nanoTime(), "creative_flight;day=6000;weather=clear");
    }

    private static void movePlayer(long now) throws Throwable {
        Object player = access.player.get(client);
        if (player == null || access.world.get(client) == null) {
            throw new IllegalStateException("World or player disappeared during benchmark");
        }
        double x = originX;
        double z = originZ;
        float yaw = -90.0f;
        long elapsed = now - sceneAt;
        if ("square_exploration".equals(scene) || "loaded_chunk_stress".equals(scene)) {
            double segment = Math.min(elapsed / (30.0 * SECOND), Math.nextDown(4.0));
            int side = (int) segment;
            double fraction = segment - side;
            double extent = "loaded_chunk_stress".equals(scene) ? 128.0 : 256.0;
            double distance = extent * smooth(fraction);
            if (side == 0) { x += distance; }
            if (side == 1) { x += extent; z += distance; }
            if (side == 2) { x += extent - distance; z += extent; }
            if (side == 3) { z += extent - distance; }
            double turn = smooth(Math.max(0.0, (fraction - 0.8) / 0.2));
            yaw = (float) (-90.0 + 90.0 * (side + turn));
        }
        playerAccess.position.invokeExact(player, x, 140.0, z, yaw, 20.0f);
        trajectory.write(scene + "," + now + "," + elapsed + "," + x + ",140.0," + z
            + "," + yaw + ",20.0");
        trajectory.newLine();
    }

    private static double smooth(double value) {
        return value * value * (3.0 - 2.0 * value);
    }

    private static void transition(String next, long now) throws IOException {
        marker("scene_end", now, scene + ";duration_ns=" + (now - sceneAt));
        scene = next;
        sceneAt = now;
        marker("scene_start", now, next);
    }

    private static void complete(Object minecraft, long now) throws ReflectiveOperationException, IOException {
        transition("complete", now);
        marker("shutdown_requested", now, "client_hook=true;server_hook=" + serverObserved);
        access.shutdown.invoke(minecraft);
        finished = true;
        shutdownRequested = true;
        flushOutputs();
    }

    private static synchronized void onShutdown() {
        try {
            if (!shutdownRequested) {
                throw new IllegalStateException("JVM exited before benchmark shutdown request");
            }
            marker("benchmark_complete", System.nanoTime(), "JVM shutdown after Minecraft shutdown");
            flushOutputs();
            Files.write(outputDirectory.resolve("complete"), new byte[0], StandardOpenOption.CREATE_NEW);
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    private static void sampleLoop() {
        try {
            while (!finished) {
                sample();
                Thread.sleep(1000L);
            }
        } catch (Throwable failure) {
            fail(failure);
        }
    }

    private static void sample() throws IOException {
        if (finished) {
            return;
        }
        long now = System.nanoTime();
        MemoryUsage heap = ManagementFactory.getMemoryMXBean().getHeapMemoryUsage();
        ClassLoadingMXBean classes = ManagementFactory.getClassLoadingMXBean();
        memory.write(scene + "," + now + "," + heap.getUsed() + "," + heap.getCommitted() + "," + heap.getMax()
            + "," + classes.getLoadedClassCount() + "," + classes.getTotalLoadedClassCount()
            + "," + classes.getUnloadedClassCount()
            + "," + ManagementFactory.getMemoryMXBean().getNonHeapMemoryUsage().getUsed()
            + "," + bufferMemory("direct") + "," + bufferMemory("mapped"));
        memory.newLine();
        flushOutputs();
    }
    private static long bufferMemory(String name) {
        for (java.lang.management.BufferPoolMXBean pool
                : ManagementFactory.getPlatformMXBeans(java.lang.management.BufferPoolMXBean.class)) {
            if (name.equals(pool.getName())) {
                return pool.getMemoryUsed();
            }
        }
        return -1L;
    }

    private static void marker(String event, long now, String detail) throws IOException {
        synchronized (events) {
            events.write(event + "," + now + ",\"" + detail.replace("\"", "\"\"")
                .replace('\n', ' ').replace('\r', ' ') + "\"");
            events.newLine();
            events.flush();
        }
    }

    private static void flushOutputs() throws IOException {
        frames.flush();
        ticks.flush();
        memory.flush();
        trajectory.flush();
        events.flush();
        synchronized (chunks) {
            chunks.flush();
        }
    }

    public static synchronized void fail(Throwable failure) {
        finished = true;
        try {
            if (outputDirectory != null) {
                java.io.StringWriter text = new java.io.StringWriter();
                failure.printStackTrace(new java.io.PrintWriter(text));
                Files.write(outputDirectory.resolve("error.txt"), text.toString().getBytes(StandardCharsets.UTF_8),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
            }
            if (events != null) {
                StringBuilder detail = new StringBuilder();
                for (Throwable cause = failure; cause != null; cause = cause.getCause()) {
                    detail.append(cause).append(';');
                    for (StackTraceElement frame : cause.getStackTrace()) {
                        detail.append(frame).append(';');
                    }
                }
                marker("error", System.nanoTime(), detail.toString());
            }
            if (frames != null && ticks != null && memory != null && trajectory != null) {
                flushOutputs();
            }
        } catch (Throwable reportingFailure) {
            failure.addSuppressed(reportingFailure);
        } finally {
            try {
                Runtime.getRuntime().halt(1);
            } catch (SecurityException trapped) {
                // FML traps halt; run.py terminates the process group once error.txt exists.
            }
        }
    }

    // MCP joined.srg from the pinned 1.12.2 SRG archive. Owner-qualified aliases
    // avoid matching an unrelated short obfuscated name on a subclass.
    private static final String[][] OFFICIAL = {
        {"field_71462_r", "bib", "m"}, {"field_71441_e", "bib", "f"},
        {"field_71439_g", "bib", "h"}, {"field_71445_n", "bib", "ag"},
        {"field_71474_y", "bib", "t"}, {"field_82881_y", "bid", "A"},
        {"field_75101_c", "aeb", "c"}, {"field_75100_b", "aeb", "b"},
        {"field_71075_bZ", "aed", "bO"}, {"field_77137_b", "amz", "b"},
        {"func_71401_C", "bib", "F"}, {"func_71218_a", "net.minecraft.server.MinecraftServer", "a"},
        {"func_175694_M", "amu", "T"}, {"func_177958_n", "fq", "p"},
        {"func_177952_p", "fq", "r"}, {"func_77146_a", "ams", "a"},
        {"func_77166_b", "amx", "b"}, {"func_71371_a", "bib", "a"},
        {"func_71016_p", "bud", "w"}, {"func_71165_d", "bud", "g"},
        {"func_70080_a", "vg", "a"}, {"func_147108_a", "bib", "a"},
        {"func_71400_g", "bib", "n"}
    };

    private static boolean officialMember(Class<?> owner, String srg, String name) {
        for (String[] mapping : OFFICIAL) {
            if (mapping[0].equals(srg) && mapping[1].equals(owner.getName())
                    && mapping[2].equals(name)) {
                return true;
            }
        }
        return false;
    }

    private static Class<?> gameClass(ClassLoader loader, String named, String official)
            throws ClassNotFoundException {
        return Class.forName("bib".equals(client.getClass().getName()) ? official : named, false, loader);
    }
    private static Field field(Class<?> type, String mcp, String srg) throws NoSuchFieldException {
        for (Class<?> current = type; current != null; current = current.getSuperclass()) {
            for (Field candidate : current.getDeclaredFields()) {
                if (candidate.getName().equals(mcp) || candidate.getName().equals(srg)
                        || officialMember(current, srg, candidate.getName())) {
                    candidate.setAccessible(true);
                    return candidate;
                }
            }
        }
        throw new NoSuchFieldException(type.getName() + ": " + mcp + "/" + srg);
    }

    private static Method method(Class<?> type, String mcp, String srg, Class<?>... arguments)
            throws NoSuchMethodException {
        for (Class<?> current = type; current != null; current = current.getSuperclass()) {
            for (Method candidate : current.getDeclaredMethods()) {
                if ((candidate.getName().equals(mcp) || candidate.getName().equals(srg)
                        || officialMember(current, srg, candidate.getName()))
                        && java.util.Arrays.equals(candidate.getParameterTypes(), arguments)) {
                    candidate.setAccessible(true);
                    return candidate;
                }
            }
        }
        throw new NoSuchMethodException(type.getName() + ": " + mcp + "/" + srg);
    }

    private static final class Timing {
        private long start;
        private int depth;
    }

    private static final class ClientAccess {
        private final Field screen;
        private final Field world;
        private final Field player;
        private final Class<?> mainMenu;
        private final Field paused;
        private final Method display;
        private final Method shutdown;

        private ClientAccess(Class<?> type) throws ReflectiveOperationException {
            screen = field(type, "currentScreen", "field_71462_r");
            world = field(type, "world", "field_71441_e");
            player = field(type, "player", "field_71439_g");
            mainMenu = gameClass(type.getClassLoader(), "net.minecraft.client.gui.GuiMainMenu", "blr");
            paused = field(type, "isGamePaused", "field_71445_n");
            display = method(type, "displayGuiScreen", "func_147108_a",
                gameClass(type.getClassLoader(), "net.minecraft.client.gui.GuiScreen", "blk"));
            shutdown = method(type, "shutdown", "func_71400_g");
        }
    }

    private static final class PlayerAccess {
        private final Field capabilities;
        private final MethodHandle position;

        private PlayerAccess(Object player) throws ReflectiveOperationException {
            Class<?> type = player.getClass();
            capabilities = field(type, "capabilities", "field_71075_bZ");
            Method setter = method(type, "setPositionAndRotation", "func_70080_a",
                double.class, double.class, double.class, float.class, float.class);
            position = MethodHandles.lookup().unreflect(setter).asType(MethodType.methodType(
                void.class, Object.class, double.class, double.class, double.class, float.class, float.class));
        }
    }

    private static final class ObserverTransformer implements ClassFileTransformer {
        private final Set<String> serverTypes = ConcurrentHashMap.newKeySet();
        private final Set<String> transformed = ConcurrentHashMap.newKeySet();

        private ObserverTransformer() {
            serverTypes.add("net/minecraft/server/MinecraftServer");
        }

        @Override
        public byte[] transform(ClassLoader loader, String name, Class<?> redefining,
                ProtectionDomain domain, byte[] bytes) {
            if (name == null || !(name.equals("net/minecraft/client/Minecraft")
                    || name.startsWith("net/minecraft/server/") || name.equals("bib") || name.equals("chd")
                    || rendererType(name))) {
                return null;
            }
            try {
                ClassReader reader = new ClassReader(bytes);
                boolean clientType = "net/minecraft/client/Minecraft".equals(name) || "bib".equals(name);
                boolean serverType = serverTypes.contains(name) || serverTypes.contains(reader.getSuperName());
                if (!clientType && !serverType && !rendererType(name)) {
                    return null;
                }
                ensureObserverVisible(loader);
                if (serverType) {
                    serverTypes.add(name);
                }
                if (!transformed.add(name)) {
                    throw new IllegalStateException("Duplicate instrumentation: " + name);
                }
                ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
                HookVisitor visitor = new HookVisitor(writer, name, clientType);
                reader.accept(visitor, ClassReader.EXPAND_FRAMES);
                if (visitor.hooks == 0 && (clientType || rendererType(name)
                        || "net/minecraft/server/MinecraftServer".equals(name))) {
                    throw new IllegalStateException("No benchmark target method in " + name);
                }
                if (visitor.hooks == 0) {
                    return null;
                }
                marker("hooks_transformed", System.nanoTime(), name + ";methods=" + visitor.hooks);
                return writer.toByteArray();
            } catch (Throwable failure) {
                fail(failure);
                return null;
            }
        }
        private static boolean rendererType(String name) {
            return name.equals("bxr") || name.equals("bxm")
                || name.equals("net/minecraft/client/renderer/chunk/RenderChunk")
                || name.equals("net/minecraft/client/renderer/chunk/ChunkRenderDispatcher");
        }

        private static void ensureObserverVisible(ClassLoader loader) throws ReflectiveOperationException {
            if (loader == null) {
                throw new IllegalStateException("Minecraft class has no application class loader");
            }
            Method exclude = null;
            try {
                exclude = loader.getClass().getMethod("addClassLoaderExclusion", String.class);
            } catch (NoSuchMethodException absent) {
                // Parent-first loaders need no exclusion; identity below still proves visibility.
            }
            if (exclude != null) {
                exclude.invoke(loader, "dev.ravel.metrics.");
            }
            if (Class.forName(CLASS_NAME, false, loader) != BenchmarkAgent.class) {
                throw new IllegalStateException("Minecraft loader defines a second benchmark observer");
            }
        }
    }

    private static final class HookVisitor extends ClassVisitor {
        private final String type;
        private final boolean clientType;
        private int hooks;

        private HookVisitor(ClassVisitor delegate, String type, boolean clientType) {
            super(Opcodes.ASM9, delegate);
            this.type = type;
            this.clientType = clientType;
        }

        @Override
        public MethodVisitor visitMethod(int accessFlags, String name, String descriptor,
                String signature, String[] exceptions) {
            MethodVisitor delegate = super.visitMethod(accessFlags, name, descriptor, signature, exceptions);
            boolean obfuscatedFrame = type.equals("bib") && name.equals("az");
            boolean obfuscatedTick = name.equals("C")
                && (type.equals("net/minecraft/server/MinecraftServer")
                    || type.equals("net/minecraft/server/integrated/IntegratedServer") || type.equals("chd"));
            boolean target = clientType
                ? name.equals("runGameLoop") || name.equals("func_71411_J") || obfuscatedFrame
                : name.equals("tick") || name.equals("func_71217_p") || obfuscatedTick;
            String rendererHook = rendererHook(type, name, descriptor);
            if ((!target || !"()V".equals(descriptor)) && rendererHook == null
                    || Modifier.isStatic(accessFlags) || Modifier.isAbstract(accessFlags)) {
                return delegate;
            }
            hooks++;
            return new HookMethod(delegate,
                rendererHook != null ? rendererHook : clientType ? "frame" : "tick");
        }

        // Feather 1.12.2 build 31 mergedv2 mappings/mappings.tiny:
        // RenderChunk.compile(FFFLChunkCompileTask;)V = bxr.b(FFFLbxl;)V.
        // Dispatcher.uploadBuffer = bxm.a(Lbuk;Lcdz;)V; uploadGlList = bxm.a(Lbuk;ILbxr;)V.
        // Time actual synchronous uploads, not upload(...), which can only enqueue a Future.
        private static String rendererHook(String type, String name, String descriptor) {
            if (type.equals("bxr") && name.equals("b") && descriptor.equals("(FFFLbxl;)V")
                    || type.equals("net/minecraft/client/renderer/chunk/RenderChunk")
                        && name.equals("func_178581_b")
                        && descriptor.equals("(FFFLnet/minecraft/client/renderer/chunk/ChunkCompileTaskGenerator;)V")) {
                return "compile";
            }
            if (type.equals("bxm") && name.equals("a")
                    && (descriptor.equals("(Lbuk;Lcdz;)V") || descriptor.equals("(Lbuk;ILbxr;)V"))
                    || type.equals("net/minecraft/client/renderer/chunk/ChunkRenderDispatcher")
                        && (name.equals("func_178506_a")
                            && descriptor.equals("(Lnet/minecraft/client/renderer/BufferBuilder;Lnet/minecraft/client/renderer/vertex/VertexBuffer;)V")
                        || name.equals("func_178510_a")
                            && descriptor.equals("(Lnet/minecraft/client/renderer/BufferBuilder;ILnet/minecraft/client/renderer/chunk/RenderChunk;)V"))) {
                return "upload";
            }
            return null;
        }
    }

    private static final class HookMethod extends MethodVisitor {
        private final String hook;
        private final Label start = new Label();
        private final Label end = new Label();
        private final Label handler = new Label();

        private HookMethod(MethodVisitor delegate, String hook) {
            super(Opcodes.ASM9, delegate);
            this.hook = hook;
        }

        @Override
        public void visitCode() {
            super.visitCode();
            mv.visitVarInsn(Opcodes.ALOAD, 0);
            mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, hook + "Enter", "(Ljava/lang/Object;)V", false);
            mv.visitLabel(start);
        }

        @Override
        public void visitInsn(int opcode) {
            if (opcode == Opcodes.RETURN) {
                // Original return frames can legally discard every local. The
                // entry hook already captured client identity and thread timing.
                mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, hook + "Exit", "()V", false);
            }
            super.visitInsn(opcode);
        }

        @Override
        public void visitMaxs(int maxStack, int maxLocals) {
            mv.visitLabel(end);
            mv.visitTryCatchBlock(start, end, handler, "java/lang/Throwable");
            mv.visitLabel(handler);
            // Existing monitor-cleanup handlers may merge local 0 to TOP. This
            // handler only consumes the exception, so require no preserved locals.
            mv.visitFrame(Opcodes.F_NEW, 0, new Object[0], 1, new Object[] { "java/lang/Throwable" });
            mv.visitInsn(Opcodes.DUP);
            mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, "fail", "(Ljava/lang/Throwable;)V", false);
            mv.visitInsn(Opcodes.ATHROW);
            super.visitMaxs(maxStack, maxLocals);
        }
    }
}
