package dev.ravel.legacy.launch;

import dev.ravel.legacy.input.Keyboard;
import dev.ravel.legacy.input.Mouse;
import dev.ravel.legacy.opengl.Display;
import dev.ravel.legacy.opengl.DisplayMode;
import dev.ravel.legacy.opengl.GeometryProof;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.Robot;
import java.awt.Toolkit;
import java.awt.event.InputEvent;
import java.awt.event.KeyEvent;
import java.awt.image.BufferedImage;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicReference;
import javax.imageio.ImageIO;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.system.MemoryStack;

/** Hooks are the official 1.12.2 obfuscated names, not Forge or LaunchWrapper. */
final class ClientAutomation implements Runnable {
    private final ClassLoader loader;
    private final Path evidence;
    private Object game;
    private Robot robot;
    private String saveName;
    private final long started = System.nanoTime();
    private long pollingDeadline;
    private long rendererBeforeReload;
    private long rendererAfterReload;
    private long rendererAfterUnload;
    private long rendererReopened;
    private boolean rendererStaleRejected;
    private boolean rendererInjectedStaleRejected;
    private long unloadingGeneration;
    private long unloadDrainChecks;
    private long unloadRetiredPoolGeneration;
    private String geometryLiveProof;

    ClientAutomation(ClassLoader loader, Path evidence) {
        this.loader = loader;
        this.evidence = evidence;
    }

    @Override
    public void run() {
        try {
            Files.createDirectories(evidence);
            Files.deleteIfExists(evidence.resolve("automation.complete"));
            robot = new Robot();
            robot.setAutoDelay(0);
            await(() -> {
                game = type("bib").getMethod("z").invoke(null);
                return game != null && field(game, "m") != null;
            });
            snapshot("01-menu");
            String server = System.getProperty("ravel.legacy.server");
            if (server != null) {
                connectDedicated(server);
                return;
            }
            createIntegratedWorld();
            AudioAcceptance.play(evidence);
            snapshot("03-world");
            walkAndLook();
            if (Boolean.getBoolean("ravel.legacy.acceptance")) stressMovement();
            if (geometrySelected() && geometryLiveProof == null) captureGeometryProof();
            wheel();
            resize();
            key(KeyEvent.VK_F11);
            await(() -> displayFlag(true));
            snapshot("05-fullscreen");
            key(KeyEvent.VK_F11);
            await(() -> displayFlag(false));
            // Vanilla restores launch dimensions, not the last resized window.
            requireDisplaySize(854, 480);
            snapshot("05b-windowed");
            key(KeyEvent.VK_ESCAPE);
            await(() -> onGame(() -> field(game, "m") != null));
            if (Boolean.getBoolean("ravel.legacy.acceptance")) requirePausedHooks();
            snapshot("06-pause");
            long drainChecks = Boolean.getBoolean("ravel.legacy.acceptance") ? meshMetric("drainChecks") : 0;
            long drainFailures = Boolean.getBoolean("ravel.legacy.acceptance") ? meshMetric("drainFailures") : 0;
            if (Boolean.getBoolean("ravel.legacy.acceptance")) unloadingGeneration = rendererMetric("currentGeneration");
            button(1);
            await(() -> onGame(() -> field(game, "f") == null));
            if (Boolean.getBoolean("ravel.legacy.acceptance")) {
                rendererAfterUnload = rendererMetric("currentGeneration");
                require(rendererAfterUnload > rendererAfterReload, "Renderer unload did not retire generation");
                rendererStaleRejected &= !rendererAccepts(rendererAfterReload);
                unloadDrainChecks = meshMetric("drainChecks") - drainChecks;
                unloadRetiredPoolGeneration = meshMetric("lastRetiredPoolGeneration");
                // The unloaded generation's own pool must report stopped, empty, unreserved final stats.
                require(unloadDrainChecks > 0 && unloadRetiredPoolGeneration == unloadingGeneration
                        && meshMetric("drainFailures") == drainFailures && meshMetric("rejectedRecords") == 0,
                        "Native geometry work of the unloaded generation did not drain at world unload");
            }
            snapshot("07-saved-menu");
            checkpoint("integrated-first.dat");
            reopen();
            checkpoint("integrated-reopened.dat");
            if (Boolean.getBoolean("ravel.legacy.acceptance")) rendererProof();
            Files.writeString(evidence.resolve("automation.complete"),
                    "seed=8675309\nactions=click-menu,type-worldname,type-seed,create,audio-playing,ticks,walk,look"
                    + (Boolean.getBoolean("ravel.legacy.acceptance") ? ",stress-teleport" : "")
                    + ",wheel,resize,fullscreen,save-quit,reopen\n"
                    + "durationMillis=" + TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started) + "\n");
            onGame(() -> invoke(game, "n", new Class<?>[0]));
        } catch (Exception failure) {
            failure.printStackTrace();
            System.exit(2);
        }
    }

    private void createIntegratedWorld() throws Exception {
            button(1);
            await(() -> onGame(() -> screenIs("bok")));
            button(3);
            await(() -> onGame(() -> screenIs("boi")));
            configureWorld();
            snapshot("02-create-world");
            if (Boolean.getBoolean("ravel.legacy.acceptance"))
                rendererType().getMethod("armStaleProof").invoke(null);
            button(0);
            await(() -> onGame(() -> field(game, "f") != null && field(game, "h") != null));
            waitTicks(100);
            requireLiveProbes();
            if (Boolean.getBoolean("ravel.legacy.acceptance")) {
                await(() -> (boolean) rendererType().getMethod("staleProofReady").invoke(null));
                await(() -> (boolean) rendererType().getMethod("snapshotProofReady").invoke(null));
                rendererBeforeReload = rendererMetric("currentGeneration");
            }
            requireSeed();
            if (Boolean.getBoolean("ravel.legacy.acceptance")) {
                Object server = onGame(() -> invoke(game, "F", new Class<?>[0]));
                double[] ground = geometrySelected() ? designateGoldenScenes(server) : null;
                reloadResources();
                if (ground != null) {
                    measureGolden(server, 1);
                    teleport(server, ground[0], ground[1], ground[2], ground);
                }
            }
    }

    private void requireLiveProbes() throws Exception {
        String probe = System.getProperty("ravel.legacy.clientProbe");
        if (probe != null) {
            Method complete = Class.forName(probe).getMethod("isComplete");
            await(() -> Boolean.TRUE.equals(complete.invoke(null)));
        }
    }

    private void connectDedicated(String address) throws Exception {
        String[] endpoint = address.split(":", 2);
        int port = endpoint.length == 2 ? Integer.parseInt(endpoint[1]) : 25565;
        onGame(() -> {
            // Pinned official ConnectScreen constructor; the ordinary game connection path.
            Object screen = type("bkr").getConstructor(type("blk"), type("bib"), String.class, int.class)
                    .newInstance(field(game, "m"), game, endpoint[0], port);
            invoke(game, "a", new Class<?>[] {type("blk")}, screen);
            return null;
        });
        await(() -> onGame(() -> field(game, "f") != null && field(game, "h") != null));
        waitTicks(100);
        requireLiveProbes();
        snapshot("dedicated-world");
        key(KeyEvent.VK_ESCAPE);
        await(() -> onGame(() -> field(game, "m") != null));
        button(1);
        await(() -> onGame(() -> field(game, "f") == null));
        Files.writeString(evidence.resolve("automation.complete"),
                "server=" + address + "\nactions=connect,real-payloads,live-probes,disconnect\n"
                + "durationMillis=" + TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started) + "\n");
        onGame(() -> invoke(game, "n", new Class<?>[0]));
    }
    private boolean screenIs(String name) throws ReflectiveOperationException {
        Object screen = field(game, "m");
        return screen != null && screen.getClass().getName().equals(name);
    }
    private void requireSeed() throws Exception {
        long seed = onGame(() -> {
            Object server = invoke(game, "F", new Class<?>[0]);
            Object world = invoke(server, "a", new Class<?>[] {int.class}, 0);
            return (long) invoke(world, "Q", new Class<?>[0]);
        });
        require(seed == 8675309L, "Integrated server did not use the fixed seed");
        Files.writeString(evidence.resolve("seed.txt"), "seed=" + seed + "\n");
    }

    private long hookCount(String id) throws ReflectiveOperationException {
        return (long) Class.forName("dev.ravel.jvm.LegacyRuntime")
                .getMethod("hookCount", String.class).invoke(null, id);
    }

    private long resourceMetric(String method) throws ReflectiveOperationException {
        return (long) Class.forName("dev.ravel.jvm.LegacyRuntime").getMethod(method).invoke(null);
    }

    private Class<?> rendererType() throws ClassNotFoundException {
        return Class.forName("dev.ravel.jvm.LegacyRenderer");
    }

    private long rendererMetric(String name) throws ReflectiveOperationException {
        return (long) rendererType().getMethod(name).invoke(null);
    }

    private boolean rendererAccepts(long generation) throws ReflectiveOperationException {
        return (boolean) rendererType().getMethod("accepts", long.class).invoke(null, generation);
    }

    private long rendererCount(int event) throws ReflectiveOperationException {
        return (long) rendererType().getMethod("count", int.class).invoke(null, event);
    }

    private void rendererProof() throws Exception {
        long finalUnload = rendererMetric("currentGeneration");
        require(rendererAfterReload > rendererBeforeReload && rendererReopened >= rendererAfterUnload
                && finalUnload > rendererReopened, "Renderer generations out of order");
        rendererStaleRejected &= !rendererAccepts(rendererReopened);
        require(rendererStaleRejected && rendererAccepts(finalUnload), "Renderer stale token accepted");
        String[] seams = {"reload", "unload", "frame", "terrain", "layer", "update",
                "compileNormalReturn", "taskCallbackNormalReturn", "upload"};
        StringBuilder counts = new StringBuilder();
        for (int event = 1; event <= 9; event++) {
            long count = rendererCount(event);
            require(count > 0, "Renderer seam not observed: " + seams[event - 1]);
            if (event > 1) counts.append(',');
            counts.append('"').append(seams[event - 1]).append("\":").append(count);
        }
        boolean worker = (boolean) rendererType().getMethod("workerObserved").invoke(null);
        require(worker, "Renderer worker thread not observed");
        require(rendererMetric("ownerThread") == onGame(() -> Thread.currentThread().threadId()),
                "Renderer owner is not the client game thread");
        String snapshots = snapshotProof();
        String mesh = meshProof();
        Files.writeString(evidence.resolve("renderer.json"),
                "{\"beforeReload\":" + rendererBeforeReload + ",\"afterReload\":" + rendererAfterReload
                + ",\"afterUnload\":" + rendererAfterUnload + ",\"reopened\":" + rendererReopened
                + ",\"finalUnload\":" + finalUnload + ",\"staleRejected\":" + rendererStaleRejected
                + ",\"ownerThread\":" + rendererMetric("ownerThread")
                + ",\"injectedStaleObservationRejected\":" + rendererInjectedStaleRejected
                + ",\"workerObserved\":" + worker + ",\"snapshot\":" + snapshots + ",\"mesh\":" + mesh
                + ",\"seams\":{" + counts + "}}\n");
    }

    private static final int STRESS_TELEPORTS = 6;
    private static final int STRESS_STEP_BLOCKS = 128;
    private static final int STRESS_MIN_DRAINED = 32;

    private long meshMetric(String name) throws ReflectiveOperationException {
        String[] names = (String[]) rendererType().getMethod("meshMetricNames").invoke(null);
        long[] values = (long[]) rendererType().getMethod("meshMetrics").invoke(null);
        for (int index = 0; index < names.length; index++) if (names[index].equals(name)) return values[index];
        throw new NoSuchFieldException(name);
    }

    // Server-authoritative teleports into ungenerated terrain force fresh section compiles and geometry jobs.
    private void stressMovement() throws Exception {
        Object server = onGame(() -> invoke(game, "F", new Class<?>[0]));
        double[] origin = onGame(this::playerState);
        boolean invulnerable = onServer(server, () -> abilities(server, true));
        StringBuilder steps = new StringBuilder();
        rendererType().getMethod("beginMeshStress").invoke(null);
        try {
            for (int step = 1; step <= STRESS_TELEPORTS; step++) {
                long drained = meshMetric("drained");
                long submitted = meshMetric("submitted");
                teleport(server, origin[0] + step * STRESS_STEP_BLOCKS, 160, origin[2], origin);
                if (step == 1 && geometrySelected()) captureGeometryProof();
                await("Fresh geometry drains after stress teleport " + step,
                        () -> meshMetric("drained") - drained >= STRESS_MIN_DRAINED, TimeUnit.MINUTES.toNanos(3));
                waitTicks(20);
                steps.append("step=").append(step).append(" submitted=").append(meshMetric("submitted") - submitted)
                        .append(" drained=").append(meshMetric("drained") - drained).append('\n');
            }
            teleport(server, origin[0], origin[1], origin[2], origin);
            waitTicks(40);
        } finally {
            rendererType().getMethod("endMeshStress").invoke(null);
            onServer(server, () -> abilities(server, invulnerable));
        }
        Files.writeString(evidence.resolve("stress.txt"), "seed=8675309 teleports=" + STRESS_TELEPORTS
                + " stepBlocks=" + STRESS_STEP_BLOCKS + " origin=" + Arrays.toString(origin) + "\n" + steps);
        snapshot("04a-stress");
    }

    // Pinned 1.12.2: server.am() PlayerManager, v() players, aed.bO abilities, aeb.a invulnerable.
    private boolean abilities(Object server, boolean invulnerable) throws ReflectiveOperationException {
        Object abilities = field(serverPlayer(server), "bO");
        Field flag = declaredField(abilities.getClass(), "a");
        boolean before = flag.getBoolean(abilities);
        flag.setBoolean(abilities, invulnerable);
        return before;
    }

    private Object serverPlayer(Object server) throws ReflectiveOperationException {
        List<?> players = (List<?>) invoke(invoke(server, "am", new Class<?>[0]), "v", new Class<?>[0]);
        require(players.size() == 1, "Integrated stress requires exactly one server player");
        return players.get(0);
    }

    // Pinned 1.12.2: ServerPlayerEntity.a networkHandler; pa.a(DDDFF) server teleport with client confirm.
    private void teleport(Object server, double x, double y, double z, double[] look) throws Exception {
        onServer(server, () -> invoke(field(serverPlayer(server), "a"), "a",
                new Class<?>[] {double.class, double.class, double.class, float.class, float.class},
                x, y, z, (float) look[3], (float) look[4]));
        await(() -> onGame(() -> {
            double[] now = playerState();
            return Math.abs(now[0] - x) < 1 && Math.abs(now[2] - z) < 1;
        }));
    }

    private <T> T onServer(Object server, Callable<T> action) throws Exception {
        var value = new AtomicReference<T>();
        Callable<Void> task = () -> {
            value.set(action.call());
            return null;
        };
        Object result = invoke(server, "a", new Class<?>[] {Callable.class}, task);
        if (!(result instanceof Future<?> future)) throw new IllegalStateException("No server task future");
        future.get(180, TimeUnit.SECONDS);
        return value.get();
    }

    private String meshProof() throws Exception {
        boolean stalePoll = onGame(() -> (boolean) rendererType().getMethod("injectStaleMeshProof", long.class)
                .invoke(null, rendererBeforeReload));
        long[] closed = onGame(() -> (long[]) rendererType().getMethod("shutdownMeshWork").invoke(null));
        if (geometrySelected()) writeGeometryProof();
        int keysHeld = (int) rendererType().getMethod("meshKeysHeld").invoke(null);
        require(stalePoll && closed.length == 18 && closed[2] == 1
                && Arrays.stream(closed, 3, 8).allMatch(value -> value == 0) && closed[14] == 0 && closed[17] == 0
                && keysHeld == 0,
                "Native geometry work was published stale or not drained at close");
        String[] names = (String[]) rendererType().getMethod("meshMetricNames").invoke(null);
        long[] values = (long[]) rendererType().getMethod("meshMetrics").invoke(null);
        require(meshMetric("drained") > 0 && meshMetric("errors") == 0 && meshMetric("rejectedRecords") == 0
                && meshMetric("captureFailures") == 0 && meshMetric("drainFailures") == 0
                && meshMetric("workerFailures") == 0
                && meshMetric("stressDrained") >= STRESS_TELEPORTS * STRESS_MIN_DRAINED,
                "Native geometry admission/drain proof missing");
        StringBuilder json = new StringBuilder("{\"geometry\":\"game-isOpaqueCube-and-isFullCube-per-state-air-key-zero-empty\","
                + "\"fidelity\":\"geometry-only-no-texture-ao-or-model-parity\","
                + "\"upload\":\"" + (geometrySelected() ? "bounded-native-wire-java-owned-offscreen-gl"
                        : "none-native-owned-until-drained-then-discarded") + "\","
                + "\"denominator\":\"admitted-native-geometry-jobs-not-vanilla-compile-tasks\","
                + "\"frameInterval\":\"observational-stress-window-frame-interval-not-mesh-attribution\","
                + "\"cancellationEvidence\":\"" + (meshMetric("retiredBeforeDrain") > 0
                        ? "actual-retired-before-drain" : "deterministic-harness-only") + "\","
                + "\"timingSampleLimit\":4096,\"keySlots\":16,\"recordWidth\":11,"
                + "\"maxRecordsPerPollLimit\":8,\"statsWidth\":18,"
                + "\"stress\":{\"seed\":8675309,\"teleports\":" + STRESS_TELEPORTS + ",\"stepBlocks\":"
                + STRESS_STEP_BLOCKS + ",\"minimumDrainedPerStep\":" + STRESS_MIN_DRAINED + "},"
                + "\"stalePollRejected\":" + stalePoll + ",\"closeDrain\":{\"stopped\":" + closed[2]
                + ",\"outstanding\":" + closed[3] + ",\"queued\":" + closed[4] + ",\"running\":" + closed[5]
                + ",\"completed\":" + closed[6] + ",\"reservedBytes\":" + closed[7]
                + ",\"draining\":" + closed[14] + ",\"workerFailure\":" + closed[17]
                + ",\"javaKeysHeld\":" + keysHeld + "},\"unloadDrain\":{\"generation\":" + unloadingGeneration
                + ",\"retiredPoolGeneration\":" + unloadRetiredPoolGeneration
                + ",\"drainChecks\":" + unloadDrainChecks + "}");
        for (int i = 0; i < names.length; i++) json.append(",\"").append(names[i]).append("\":").append(values[i]);
        String[] series = (String[]) rendererType().getMethod("meshSeries").invoke(null);
        for (int s = 0; s < series.length; s++) for (int percent : new int[] {50, 95, 99}) {
            long nanos = (long) rendererType().getMethod("meshPercentile", int.class, int.class).invoke(null, s, percent);
            json.append(",\"").append(series[s]).append('P').append(percent).append("Nanos\":").append(nanos);
        }
        return json.append('}').toString();
    }

    private static boolean geometrySelected() {
        return "geometry-proof".equals(System.getProperty("ravel.renderer", "vanilla"));
    }

    private String geometryMetrics() throws ReflectiveOperationException {
        String[] names = (String[]) rendererType().getMethod("geometryProofMetricNames").invoke(null);
        long[] values = (long[]) rendererType().getMethod("geometryProofMetrics").invoke(null);
        require(names.length == values.length, "Geometry metric schema mismatch");
        StringBuilder json = new StringBuilder("{");
        for (int i = 0; i < names.length; i++) {
            if (i > 0) json.append(',');
            json.append('"').append(names[i]).append("\":").append(values[i]);
        }
        return json.append('}').toString();
    }

    // Copy all GL-owned proof state on the game thread while this world's buffers are still live.
    private void captureGeometryProof() throws Exception {
        AtomicReference<byte[][]> captured = new AtomicReference<>();
        AtomicReference<long[]> lastComparison = new AtomicReference<>();
        try {
            await("Published same-section vanilla geometry comparison", () -> onGame(() -> {
                long[] vanilla = (long[]) rendererType().getMethod("geometryProofComparePublishedVanilla").invoke(null);
                require(vanilla.length == 9, "Vanilla comparison schema mismatch");
                lastComparison.set(vanilla);
                if (vanilla[0] != 1) return false;
                String[] comparisonNames = {"status", "generation", "key", "vanillaVertices", "vanillaLayers",
                        "rgbaMismatchPixels", "coverageMismatchPixels", "depthMismatchPixels", "totalPixels"};
                StringBuilder comparison = new StringBuilder("{");
                for (int i = 0; i < vanilla.length; i++) {
                    if (i > 0) comparison.append(',');
                    comparison.append('"').append(comparisonNames[i]).append("\":").append(vanilla[i]);
                }
                comparison.append('}');
                String[] info = (String[]) rendererType().getMethod("geometryProofInfo").invoke(null);
                require(info.length == 3 && info[0].equals("geometry-proof"), "Geometry mode not selected");
                byte[][] pixels = {
                    (byte[]) rendererType().getMethod("geometryProofPixels").invoke(null),
                    (byte[]) rendererType().getMethod("geometryProofReferencePixels").invoke(null),
                    (byte[]) rendererType().getMethod("geometryProofVanillaPixels").invoke(null)};
                int nativeForeground = geometryForeground(pixels[0]);
                int vanillaForeground = geometryForeground(pixels[2]);
                if (nativeForeground == 0 || vanillaForeground == 0) {
                    vanilla[0] = -6;
                    return false;
                }
                comparison.setLength(comparison.length() - 1);
                comparison.append(",\"nativeForegroundPixels\":").append(nativeForeground)
                        .append(",\"vanillaForegroundPixels\":").append(vanillaForeground).append('}');
                geometryLiveProof = "{\"selection\":\"geometry-proof\",\"offDefault\":true,"
                        + "\"worldSubstitution\":false,\"fidelity\":\"not-proven\","
                        + "\"reference\":\"self-reference-not-vanilla-oracle\","
                        + "\"vanillaScope\":\"published-SOLID-only-section-position-coverage-depth\","
                        + "\"width\":64,\"height\":64,\"rowOrder\":\"bottom-up\","
                        + "\"info\":[\"" + String.join("\",\"", info) + "\"],\"metrics\":" + geometryMetrics()
                        + ",\"vanillaComparison\":" + comparison
                        + ",\"shadedNativeLive\":" + shadedResidency("shadedNativeResidency")
                        + ",\"shadedGlLive\":" + shadedResidency("shadedGlResidency");
                captured.set(pixels);
                return true;
            }), TimeUnit.MINUTES.toNanos(3));
        } catch (IllegalStateException failure) {
            throw new IllegalStateException(failure.getMessage()
                    + " lastVanillaComparison[status,generation,key,vertices,layers,rgbaMismatch,"
                    + "coverageMismatch,depthMismatch,totalPixels]=" + Arrays.toString(lastComparison.get())
                    + " lastStatus=" + vanillaStatus(lastComparison.get()), failure);
        }
        byte[][] images = captured.get();
        String[] imageNames = {"native", "reference", "vanilla"};
        for (int i = 0; i < images.length; i++) {
            byte[] rgba = images[i];
            require(rgba != null && rgba.length == 64 * 64 * 4, "Geometry proof pixels unavailable before unload");
            String name = "geometry-" + imageNames[i];
            Files.write(evidence.resolve(name + ".rgba"), rgba);
            BufferedImage image = new BufferedImage(64, 64, BufferedImage.TYPE_INT_ARGB);
            for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) {
                int offset = ((63 - y) * 64 + x) * 4;
                image.setRGB(x, y, (rgba[offset + 3] & 255) << 24 | (rgba[offset] & 255) << 16
                        | (rgba[offset + 1] & 255) << 8 | (rgba[offset + 2] & 255));
            }
            require(ImageIO.write(image, "png", evidence.resolve(name + ".png").toFile()),
                    "Geometry PNG writer unavailable");
        }
    }

    private static int geometryForeground(byte[] rgba) {
        require(rgba != null && rgba.length == 64 * 64 * 4, "Geometry proof pixel schema mismatch");
        int foreground = 0;
        for (int at = 0; at < rgba.length; at += 4) if (rgba[at] != 0) foreground++;
        return foreground;
    }

    private static String vanillaStatus(long[] comparison) {
        if (comparison == null || comparison.length == 0) return "absent-not-polled";
        return switch ((int) comparison[0]) {
            case 1 -> "compared";
            case 0 -> "absent-resident";
            case -1 -> "stale-task-publication-origin-or-generation";
            case -2 -> "busy-task-or-compile-lock";
            case -3 -> "invalid-layer-buffer-format-or-readback";
            case -4 -> "mixedscope-or-nonsolid-section";
            case -5 -> "absent-same-section-association";
            case -6 -> "absent-foreground-in-current-projection";
            default -> "unknown-" + comparison[0];
        };
    }

    private void writeGeometryProof() throws Exception {
        require(geometryLiveProof != null, "Geometry proof was not captured before unload");
        String finalMetrics = onGame(this::geometryMetrics);
        Files.writeString(evidence.resolve("geometry-upload.json"),
                geometryLiveProof + ",\"finalMetrics\":" + finalMetrics + ",\"baked\":" + bakedProof()
                        + ",\"shadedNativeResidency\":" + onGame(() -> shadedResidency("shadedNativeResidency"))
                        + ",\"shadedGlResidency\":" + onGame(() -> shadedResidency("shadedGlResidency")) + "}\n");
    }

    private String shadedResidency(String name) throws Exception {
        return Arrays.toString((long[]) rendererType().getMethod(name).invoke(null));
    }

    private static final String[] BAKED_NAMES = {"comparedSections", "busySections", "unsupportedSections",
            "staleSections", "quads", "mismatchedWords", "cpuNanos", "maxInputBytes", "maxOutputBytes",
            "animatedSpriteObservations", "firstMismatchedWord", "resourceGeneration", "atlasGeneration",
            "glComparedSections", "glStaleSections", "glUnsupportedSections",
            "glUploadedBytes", "glPeakBufferBytes", "glUploadNanos", "glComparisonNanos",
            "refusedQuadBudget", "refusedUncolored", "refusedForeignBuilder", "refusedObservation",
            "refusedIncompleteQuad", "refusedUnaccountedVertices", "refusedRetiredAtFinish",
            "refusedRustDecode", "refusedRustOther", "controlledDeferredSections"};
    private static final String[] BAKED_SCENES = {"opaque", "cutout-mipped", "cutout", "translucent",
            "animated", "staged-reload", "unload", "translucent-fluid"};

    private String bakedProof() throws Exception {
        long[] values = (long[]) rendererType().getMethod("bakedMetrics").invoke(null);
        require(values.length == BAKED_NAMES.length, "Baked metric schema mismatch");
        require(values[0] > 0 && values[4] > 0 && values[5] == 0 && values[10] == -1,
                "Baked CPU comparison missing or differs from vanilla");
        StringBuilder json = new StringBuilder("{\"comparison\":\"Rust-emission-vs-final-vanilla-BLOCK-words\","
                + "\"drawable\":true,\"worldSubstitution\":false,\"maxQuadsPerSection\":" + GeometryProof.MAX_BAKED_QUADS);
        for (int i = 0; i < BAKED_NAMES.length; i++)
            json.append(",\"").append(BAKED_NAMES[i]).append("\":").append(values[i]);
        json.append(",\"world\":").append(Arrays.toString((long[]) rendererType().getMethod("bakedWorld").invoke(null)));
        long[][] goldens = (long[][]) rendererType().getMethod("bakedGoldens").invoke(null);
        require(goldens.length == BAKED_SCENES.length, "Baked golden schema mismatch");
        json.append(",\"goldens\":[");
        for (int scene = 0; scene < BAKED_SCENES.length; scene++) {
            if (scene > 0) json.append(',');
            json.append(goldenRow(scene, goldens[scene]));
        }
        return json.append("]}").toString();
    }

    private String goldenRow(int scene, long[] row) throws Exception {
        byte[][] samples = (byte[][]) rendererType().getMethod("bakedReadbacks", int.class).invoke(null, scene);
        StringBuilder files = new StringBuilder();
        for (int index = 0; index < samples.length; index++) {
            String name = "baked-golden-" + scene + "-" + index + ".raw";
            Files.write(evidence.resolve(name), samples[index]);
            files.append(index > 0 ? ",\"" : "\"").append(name).append('"');
        }
        boolean measured = row[0] > 0 && row[4] > 0 && row[5] > 0;
        String reason = scene == 7 ? "fluid-tessellation-not-baked-quad-provenance"
                : row[0] == 0 ? "no-admitted-controlled-section"
                : !measured ? "no-foreground-in-section-local-negative-z-projection"
                : "controlled-section-rust-emission-vs-final-vanilla-buffer-offscreen";
        return "{\"scene\":\"" + BAKED_SCENES[scene] + "\",\"status\":\"" + (measured ? "measured" : "unsupported")
                + "\",\"reason\":\"" + reason + "\",\"values\":" + Arrays.toString(row)
                + ",\"readbacks\":[" + files + "]}";
    }

    private static final int GOLDEN_Y = 192;
    private static final String[][] GOLDEN_BLOCKS = {
            {"minecraft:stone 0", "minecraft:planks 0"}, {"minecraft:iron_bars 0", "minecraft:glass_pane 0"},
            {"minecraft:glass 0", "minecraft:web 0"}, {"minecraft:stained_glass 14", "minecraft:ice 0"},
            {"minecraft:sea_lantern 0", "minecraft:magma 0"}};
    private static final int[][] GOLDEN_CHUNKS = {{1, -1}, {1, 0}, {1, 1}, {0, -1}, {0, 1}};
    private int[] goldenOrigin;

    // Controlled arrangements occupy separate sky sections around the viewing platform.
    private double[] designateGoldenScenes(Object server) throws Exception {
        double[] ground = onGame(this::playerState);
        goldenOrigin = new int[]{Math.floorDiv((int) Math.floor(ground[0]), 16) * 16,
                Math.floorDiv((int) Math.floor(ground[2]), 16) * 16};
        for (int scene = 0; scene < GOLDEN_BLOCKS.length; scene++)
            rendererType().getMethod("bakedDesignate", int.class, int.class, int.class, int.class)
                    .invoke(null, scene, goldenOrigin[0] + GOLDEN_CHUNKS[scene][0] * 16,
                            GOLDEN_Y, goldenOrigin[1] + GOLDEN_CHUNKS[scene][1] * 16);
        int placed = onServer(server, () -> command(server, "setblock " + (goldenOrigin[0] + 8) + " "
                + (GOLDEN_Y + 7) + " " + (goldenOrigin[1] + 8) + " minecraft:stone 0"));
        require(placed > 0, "Golden viewing platform was not placed");
        measureGolden(server, 0);
        return ground;
    }

    private void measureGolden(Object server, int phase) throws Exception {
        if (phase > 0) rendererType().getMethod("bakedPhase", int.class).invoke(null, phase);
        teleport(server, goldenOrigin[0] + 8.5, GOLDEN_Y + 8, goldenOrigin[1] + 8.5, new double[]{0, 0, 0, -90, 0});
        long interval = TimeUnit.SECONDS.toNanos(3);
        long[] refilled = {System.nanoTime() - interval};
        int all = (1 << GOLDEN_BLOCKS.length) - 1;
        try {
            await("Controlled baked golden scenes measured in phase " + phase, () -> {
                int covered = goldenCoverage(phase);
                if (covered == all) return true;
                if (System.nanoTime() - refilled[0] >= interval) {
                    int scene = Integer.numberOfTrailingZeros(~covered & all);
                    double yaw = Math.toDegrees(Math.atan2(-GOLDEN_CHUNKS[scene][0], GOLDEN_CHUNKS[scene][1]));
                    teleport(server, goldenOrigin[0] + 8.5, GOLDEN_Y + 8, goldenOrigin[1] + 8.5,
                            new double[]{0, 0, 0, yaw, 0});
                    onServer(server, () -> refillGolden(server, scene));
                    refilled[0] = System.nanoTime();
                }
                return false;
            }, TimeUnit.MINUTES.toNanos(3));
        } catch (IllegalStateException failure) {
            throw new IllegalStateException(failure.getMessage() + " coverage="
                    + Integer.toBinaryString(goldenCoverage(phase)) + " baked="
                    + Arrays.toString((long[]) rendererType().getMethod("bakedMetrics").invoke(null)), failure);
        }
    }

    private int goldenCoverage(int phase) throws ReflectiveOperationException {
        return (int) rendererType().getMethod("bakedCoverage", int.class).invoke(null, phase);
    }

    // Clearing then refilling one section in a server task republishes it and forces a fresh compile.
    private Void refillGolden(Object server, int scene) throws Exception {
        int x = goldenOrigin[0] + GOLDEN_CHUNKS[scene][0] * 16;
        int z = goldenOrigin[1] + GOLDEN_CHUNKS[scene][1] * 16;
        String low = " " + (GOLDEN_Y + 6) + " " + (z + 6) + " ";
        String high = " " + (GOLDEN_Y + 9) + " " + (z + 9) + " ";
        command(server, "fill " + (x + 6) + low + (x + 9) + high + "minecraft:air 0");
        for (int half = 0; half < 2; half++)
            require(command(server, "fill " + (x + 6 + half * 2) + low + (x + 7 + half * 2) + high
                    + GOLDEN_BLOCKS[scene][half]) > 0, "Golden scene fill failed: " + GOLDEN_BLOCKS[scene][half]);
        return null;
    }

    // Pinned 1.12.2: MinecraftServer.N() command handler, bl.a(bn, String) executes as the server source.
    private int command(Object server, String text) throws ReflectiveOperationException {
        return (int) invoke(invoke(server, "N", new Class<?>[0]), "a",
                new Class<?>[] {type("bn"), String.class}, server, text);
    }

    private String snapshotProof() throws Exception {
        long[] values = (long[]) rendererType().getMethod("snapshotMetrics").invoke(null);
        String[] names = {"captures", "totalBytes", "maxBytes", "totalNanos", "maxNanos",
                "mismatches", "cells", "rejected", "skipped", "borderCells", "lastThread",
                "lastGeneration", "nonAirCells", "adjacentVariation", "maxNativeCopyBytes",
                "maxNativeScratchBytes", "staleCaptureRejections", "workerCaptures",
                "oracleChecks", "oracleMismatches", "oracleBorderChecks",
                "oracleRetryCells", "oracleRetryCandidates", "fullOpaqueCells"};
        require(values.length == names.length && values[0] > 0 && values[1] > 0
                && values[2] > 0 && values[3] > 0 && values[4] > 0 && values[5] == 0
                && values[6] == values[0] * 5832 && values[9] == values[0] * 1736
                && values[10] > 0 && values[11] > 0 && values[12] > 0 && values[13] > 0
                && values[14] == 34992 && values[15] > 34992 && values[16] > 0 && values[17] > 0
                && values[18] == 125 && values[19] == 0 && values[20] == 98
                && values[22] >= 0 && values[21] >= values[22] && values[21] <= values[22] * 125
                && values[23] > 0,
                "Snapshot proof missing bounded vanilla nonair/border/geometry roundtrip or stale rejection");
        StringBuilder json = new StringBuilder("{\"seed\":8675309,\"edge\":18,"
                + "\"comparison\":\"decoded-vs-Java-copied-post-compile-region\","
                + "\"light\":\"raw-stored-sky-and-block-not-combined-mesh-light\","
                + "\"oracle\":\"coordinate-only-same-mappings\","
                + "\"modelKeys\":\"state-derived-not-baked-model-identities\","
                + "\"geometry\":\"captured-game-isOpaqueCube-and-isFullCube\","
                + "\"timingSampleLimit\":4096,\"maxConcurrentCaptures\":1,"
                + "\"javaBatchBytes\":34992,\"retainedStaleProofBytes\":34992,"
                + "\"maxJavaBatchPayloadBytes\":69984");
        for (int i = 0; i < names.length; i++)
            json.append(",\"").append(names[i]).append("\":").append(values[i]);
        for (int percent : new int[] {50, 95, 99}) {
            long nanos = (long) rendererType().getMethod("snapshotPercentile", int.class).invoke(null, percent);
            json.append(",\"p").append(percent).append("Nanos\":").append(nanos);
        }
        return json.append('}').toString();
    }

    private void reloadResources() throws Exception {
        long reloads = hookCount("legacy.resource-reload");
        long atlases = hookCount("legacy.atlas-reload");
        long completed = resourceMetric("completedResourceGeneration");
        robot.keyPress(KeyEvent.VK_F3);
        try {
            await(() -> onGame(() -> Keyboard.isKeyDown(61)));
            key(KeyEvent.VK_T);
            await(() -> hookCount("legacy.resource-reload") > reloads
                    && hookCount("legacy.atlas-reload") > atlases
                    && resourceMetric("completedResourceGeneration") > completed
                    && resourceMetric("atlasResourceGeneration") == resourceMetric("completedResourceGeneration"));
        } finally {
            robot.keyRelease(KeyEvent.VK_F3);
        }
        Files.writeString(evidence.resolve("resource-generations.txt"),
                "reloads=" + hookCount("legacy.resource-reload")
                + " atlases=" + hookCount("legacy.atlas-reload")
                + " completedGeneration=" + resourceMetric("completedResourceGeneration")
                + " atlasGeneration=" + resourceMetric("atlasResourceGeneration") + "\n");
        rendererAfterReload = rendererMetric("currentGeneration");
        rendererStaleRejected = !rendererAccepts(rendererBeforeReload);
        require(rendererStaleRejected, "Renderer reload accepted retired generation");
        rendererInjectedStaleRejected = onGame(() ->
                (boolean) rendererType().getMethod("injectStaleProof").invoke(null));
        require(rendererInjectedStaleRejected, "Injected old-task normal return was not rejected");
        await(() -> (boolean) rendererType().getMethod("injectStaleSnapshotProof").invoke(null));
    }

    private void requirePausedHooks() throws Exception {
        long initial = hookCount("legacy.integrated-server-tick");
        await(() -> hookCount("legacy.integrated-server-tick") >= initial + 10);
        long base = hookCount("legacy.server-tick");
        long client = hookCount("legacy.client-tick");
        long overrides = hookCount("legacy.integrated-server-tick");
        long next = overrides + 10;
        await(() -> hookCount("legacy.integrated-server-tick") >= next
                && hookCount("legacy.client-tick") > client);
        require(hookCount("legacy.server-tick") == base,
                "paused integrated server dispatched a simulation tick");
        Files.writeString(evidence.resolve("paused-hooks.txt"),
                "baseBefore=" + base + " baseAfter=" + hookCount("legacy.server-tick")
                + " integratedBefore=" + overrides
                + " integratedAfter=" + hookCount("legacy.integrated-server-tick") + "\n");
    }


    private void configureWorld() throws Exception {
        typeField("g", "Ravel Legacy 8675309");
        Files.writeString(evidence.resolve("world-name.txt"), "typed=" + onGame(() -> text("g")) + "\n");
        saveName = onGame(() -> (String) field(field(game, "m"), "i"));
        button(2);
        await(() -> onGame(() -> "hardcore".equals(field(field(game, "m"), "s"))));
        button(2);
        await(() -> onGame(() -> "creative".equals(field(field(game, "m"), "s"))));
        button(3);
        await(() -> onGame(() -> (boolean) field(field(game, "m"), "A")));
        typeField("h", "8675309");
        Files.writeString(evidence.resolve("seed-input.txt"), "typed=" + onGame(() -> text("h")) + "\n");
        button(3);
        await(() -> onGame(() -> !(boolean) field(field(game, "m"), "A")));
    }

    private void typeField(String fieldName, String value) throws Exception {
        Point point = onGame(() -> {
            Object text = field(field(game, "m"), fieldName);
            return guiPoint((int) field(text, "a") + (int) field(text, "i") / 2,
                    (int) field(text, "f") + (int) field(text, "j") / 2);
        });
        click(point);
        awaitField("text focus", fieldName,
                () -> onGame(() -> (boolean) invoke(field(field(game, "m"), fieldName), "m", new Class<?>[0])));
        robot.keyPress(KeyEvent.VK_CONTROL);
        try {
            awaitField("Ctrl press (key 29)", fieldName, () -> onGame(() -> Keyboard.isKeyDown(29)));
            key(KeyEvent.VK_A);
            awaitField("Ctrl+A selection", fieldName,
                    () -> onGame(() -> invoke(field(field(game, "m"), fieldName), "c", new Class<?>[0])
                            .equals(text(fieldName))));
        } finally {
            robot.keyRelease(KeyEvent.VK_CONTROL);
        }
        awaitField("Ctrl release (key 29)", fieldName, () -> onGame(() -> !Keyboard.isKeyDown(29)));
        key(KeyEvent.VK_BACK_SPACE);
        awaitField("Backspace clear", fieldName, () -> onGame(() -> text(fieldName).isEmpty()));
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            boolean shifted = Character.isUpperCase(character);
            if (shifted) robot.keyPress(KeyEvent.VK_SHIFT);
            try {
                key(KeyEvent.getExtendedKeyCodeForChar(Character.toUpperCase(character)));
            } finally {
                if (shifted) robot.keyRelease(KeyEvent.VK_SHIFT);
            }
            String expected = value.substring(0, index + 1);
            awaitField("character " + index + " expected=[" + expected + "]", fieldName,
                    () -> onGame(() -> text(fieldName).equals(expected)));
        }
    }

    private void awaitField(String stage, String fieldName, Callable<Boolean> predicate) throws Exception {
        var lastState = new AtomicReference<>(" actualText=<unavailable> key29=<unavailable>");
        try {
            await("Text field " + fieldName + " " + stage, () -> {
                boolean complete = predicate.call();
                if (!complete) {
                    lastState.set(onGame(() -> " actualText=[" + text(fieldName) + "] key29="
                            + Keyboard.isKeyDown(29) + " selection=["
                            + invoke(field(field(game, "m"), fieldName), "c", new Class<?>[0]) + "]"));
                }
                return complete;
            }, TimeUnit.SECONDS.toNanos(10));
        } catch (IllegalStateException failure) {
            throw new IllegalStateException(failure.getMessage() + lastState.get(), failure);
        }
    }

    private String text(String fieldName) throws ReflectiveOperationException {
        return (String) invoke(field(field(game, "m"), fieldName), "b", new Class<?>[0]);
    }

    private void button(int id) throws Exception {
        Point point = onGame(() -> {
            Object screen = field(game, "m");
            Object buttons = declaredField(type("blk"), "n").get(screen);
            if (!(buttons instanceof List<?> list)) throw new IllegalStateException("No GUI button list");
            for (Object button : list) {
                if ((int) field(button, "k") == id) {
                    require((boolean) field(button, "l") && (boolean) field(button, "m"),
                            "Button is disabled or hidden: " + id);
                    return guiPoint((int) field(button, "h") + (int) field(button, "f") / 2,
                            (int) field(button, "i") + (int) field(button, "g") / 2);
                }
            }
            throw new IllegalStateException("Missing button " + id + " on " + screen.getClass());
        });
        click(point);
    }

    private Point guiPoint(int x, int y) throws ReflectiveOperationException {
        Object screen = field(game, "m");
        Rectangle bounds = windowBounds();
        return new Point(bounds.x + x * bounds.width / (int) field(screen, "l"),
                bounds.y + y * bounds.height / (int) field(screen, "m"));
    }

    private void click(Point point) throws Exception {
        onGame(() -> {
            GLFW.glfwFocusWindow(Display.window());
            return null;
        });
        await("Window focus before Robot click at " + point,
                () -> onGame(() -> GLFW.glfwGetWindowAttrib(Display.window(), GLFW.GLFW_FOCUSED) != 0),
                TimeUnit.SECONDS.toNanos(10));
        robot.mouseMove(point.x, point.y);
        robot.mousePress(InputEvent.BUTTON1_DOWN_MASK);
        try {
            await("Robot mouse press callback at " + point, () -> onGame(() -> Mouse.isButtonDown(0)),
                    TimeUnit.SECONDS.toNanos(180));
        } finally {
            robot.mouseRelease(InputEvent.BUTTON1_DOWN_MASK);
        }
        await("Robot mouse release callback at " + point, () -> onGame(() -> !Mouse.isButtonDown(0)),
                TimeUnit.SECONDS.toNanos(180));
    }

    private void walkAndLook() throws Exception {
        onGame(() -> invoke(game, "o", new Class<?>[0]));
        recordPlayer("before-walk");
        double[] before = onGame(this::playerState);
        robot.keyPress(KeyEvent.VK_W);
        try {
            waitTicks(60);
            var size = Toolkit.getDefaultToolkit().getScreenSize();
            robot.mouseMove(size.width / 2 + 120, size.height / 2 - 50);
            waitTicks(30);
        } finally {
            robot.keyRelease(KeyEvent.VK_W);
        }
        recordPlayer("after-walk-look");
        double[] after = onGame(this::playerState);
        require(Math.hypot(after[0] - before[0], after[2] - before[2]) > 0.05, "Robot walk did not move player");
        require(Math.abs(after[3] - before[3]) > 0.1, "Robot look did not change yaw");
        snapshot("04-walk-look");
    }
    private void recordPlayer(String phase) throws Exception {
        String state = onGame(() -> {
            Object player = field(game, "h");
            StringBuilder result = new StringBuilder(phase).append(" tick=").append(ticks());
            for (String name : new String[] {"p", "q", "r", "v", "w"}) {
                result.append(' ').append(name).append('=').append(declaredField(type("vg"), name).get(player));
            }
            return result.append('\n').toString();
        });
        Files.writeString(evidence.resolve("player-states.txt"), state,
                java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
    }


    private void resize() throws Exception {
        onGame(() -> {
            Display.setDisplayMode(new DisplayMode(960, 540));
            return null;
        });
        requireDisplaySize(960, 540);
        snapshot("04b-resized");
    }

    private void waitTicks(long count) throws Exception {
        long start = onGame(this::ticks);
        await(() -> onGame(this::ticks) - start >= count);
        Files.writeString(evidence.resolve("ticks.txt"), "waited=" + count + " actual=" + (onGame(this::ticks) - start) + "\n",
                java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
    }

    private long ticks() throws Exception {
        Object world = field(game, "f");
        return (long) invoke(world, "R", new Class<?>[0]);
    }
    private double[] playerState() throws Exception {
        Object player = field(game, "h");
        double[] result = new double[5];
        String[] names = {"p", "q", "r", "v", "w"};
        for (int index = 0; index < names.length; index++) {
            result[index] = ((Number) declaredField(type("vg"), names[index]).get(player)).doubleValue();
        }
        return result;
    }

    private void reopen() throws Exception {
        button(1);
        await(() -> onGame(() -> screenIs("bok")));
        Point row = onGame(() -> {
            Object list = field(field(game, "m"), "v");
            Object first = invoke(list, "c", new Class<?>[] {int.class}, 0);
            Object summary = field(first, "g");
            require(saveName.equals(invoke(summary, "a", new Class<?>[0])), "Newest save is not acceptance world");
            return guiPoint((int) field(list, "b") / 2,
                    (int) field(list, "d") + 4 + (int) field(list, "h") / 2);
        });
        click(row);
        await(() -> onGame(() -> invoke(field(field(game, "m"), "v"), "f", new Class<?>[0]) != null));
        button(1);
        await(() -> onGame(() -> field(game, "f") != null && field(game, "h") != null));
        if (Boolean.getBoolean("ravel.legacy.acceptance")) {
            rendererReopened = rendererMetric("currentGeneration");
            require(rendererAccepts(rendererReopened) && !rendererAccepts(rendererAfterReload),
                    "Renderer reopen accepted old generation");
            if (geometrySelected()) {
                Object server = onGame(() -> invoke(game, "F", new Class<?>[0]));
                double[] ground = onGame(this::playerState);
                measureGolden(server, 2);
                teleport(server, ground[0], ground[1], ground[2], ground);
            }
        }
        requireSeed();
        snapshot("08-reopened");
        key(KeyEvent.VK_ESCAPE);
        await(() -> onGame(() -> field(game, "m") != null));
        button(1);
        await(() -> onGame(() -> field(game, "f") == null));
    }

    private void checkpoint(String name) throws Exception {
        if (!Boolean.getBoolean("ravel.legacy.acceptance")) return;
        Path source = Path.of("saves", saveName, "data", "ravel_mods.dat");
        Files.copy(source, evidence.resolve(name), StandardCopyOption.REPLACE_EXISTING);
    }

    private boolean displayFlag(boolean expected) throws Exception {
        return onGame(() -> {
            boolean fullscreen = GLFW.glfwGetWindowMonitor(Display.window()) != 0;
            return fullscreen == expected && Display.isFullscreen() == expected
                    && (boolean) field(game, "T") == expected;
        });
    }

    private void requireDisplaySize(int width, int height) throws Exception {
        await(() -> onGame(() -> {
            Rectangle bounds = windowBounds();
            return Display.getWidth() == width && Display.getHeight() == height
                    && bounds.width == width && bounds.height == height
                    && (int) field(game, "d") == width && (int) field(game, "e") == height;
        }));
    }

    private void wheel() throws Exception {
        long before = onGame(Mouse::getScrollEventCount);
        int selected = onGame(this::selectedSlot);
        robot.mouseWheel(1);
        await(() -> onGame(() -> Mouse.getScrollEventCount() > before && selectedSlot() == (selected + 1) % 9));
        Files.writeString(evidence.resolve("wheel.txt"), "callbacks="
                + (onGame(Mouse::getScrollEventCount) - before) + " slotBefore=" + selected
                + " slotAfter=" + onGame(this::selectedSlot) + "\n");
    }

    private int selectedSlot() throws ReflectiveOperationException {
        return (int) field(field(field(game, "h"), "bv"), "d");
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new IllegalStateException(message);
    }


    private <T> T onGame(Callable<T> action) throws Exception {
        var value = new AtomicReference<T>();
        Callable<Void> task = () -> {
            value.set(action.call());
            return null;
        };
        Object result = type("bib").getMethod("a", Callable.class).invoke(game, task);
        if (!(result instanceof Future<?> future)) throw new IllegalStateException("No scheduled task future");
        long remaining = pollingDeadline == 0 ? TimeUnit.SECONDS.toNanos(180)
                : Math.min(TimeUnit.SECONDS.toNanos(180), pollingDeadline - System.nanoTime());
        try {
            if (remaining <= 0) throw new TimeoutException("Game-thread polling deadline expired");
            future.get(remaining, TimeUnit.NANOSECONDS);
        } catch (TimeoutException | InterruptedException failure) {
            future.cancel(false);
            throw failure;
        }
        return value.get();
    }

    private void await(Callable<Boolean> predicate) throws Exception {
        await("Automation condition", predicate, TimeUnit.MINUTES.toNanos(5));
    }

    private void await(String condition, Callable<Boolean> predicate, long timeoutNanos) throws Exception {
        long previousDeadline = pollingDeadline;
        long now = System.nanoTime();
        long budget = previousDeadline == 0 ? timeoutNanos : Math.min(timeoutNanos, previousDeadline - now);
        pollingDeadline = now + budget;
        try {
            while (true) {
                if (pollingDeadline - System.nanoTime() <= 0) throw new TimeoutException(condition);
                if (predicate.call()) return;
                long remaining = pollingDeadline - System.nanoTime();
                if (remaining <= 0) throw new TimeoutException(condition);
                TimeUnit.NANOSECONDS.sleep(Math.min(TimeUnit.MILLISECONDS.toNanos(100), remaining));
            }
        } catch (InterruptedException failure) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(condition + " interrupted", failure);
        } catch (Exception failure) {
            throw new IllegalStateException(condition + " failed: " + failure, failure);
        } finally {
            pollingDeadline = previousDeadline;
        }
    }

    private void snapshot(String name) throws Exception {
        Rectangle bounds = onGame(this::windowBounds);
        ImageIO.write(robot.createScreenCapture(bounds), "png", evidence.resolve(name + ".png").toFile());
        System.out.println("RAVEL_AUTOMATION " + name);
    }

    private Rectangle windowBounds() {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            var x = stack.mallocInt(1);
            var y = stack.mallocInt(1);
            var width = stack.mallocInt(1);
            var height = stack.mallocInt(1);
            long window = Display.window();
            GLFW.glfwGetWindowPos(window, x, y);
            // Robot takes screen coordinates, not high-DPI framebuffer pixels.
            GLFW.glfwGetWindowSize(window, width, height);
            return new Rectangle(x.get(0), y.get(0), width.get(0), height.get(0));
        }
    }

    private void key(int code) {
        robot.keyPress(code);
        robot.keyRelease(code);
    }

    private Class<?> type(String name) throws ClassNotFoundException {
        return loader.loadClass(name);
    }

    private static Field declaredField(Class<?> owner, String name) throws NoSuchFieldException {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Object field(Object owner, String name) throws ReflectiveOperationException {
        for (Class<?> cursor = owner.getClass(); cursor != null; cursor = cursor.getSuperclass()) {
            try {
                return declaredField(cursor, name).get(owner);
            } catch (NoSuchFieldException absent) {
                // Obfuscated fields can belong to an entity or screen superclass.
            }
        }
        throw new NoSuchFieldException(owner.getClass().getName() + "." + name);
    }


    private static Object invoke(Object owner, String name, Class<?>[] signature, Object... args)
            throws ReflectiveOperationException {
        for (Class<?> cursor = owner.getClass(); cursor != null; cursor = cursor.getSuperclass()) {
            try {
                Method method = cursor.getDeclaredMethod(name, signature);
                method.setAccessible(true);
                return method.invoke(owner, args);
            } catch (NoSuchMethodException absent) {
                // Resolve the actual inherited legacy member, not a name-only overload.
            }
        }
        throw new NoSuchMethodException(owner.getClass().getName() + "." + name);
    }
}
