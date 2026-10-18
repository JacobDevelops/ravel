package dev.ravel.legacy.launch;

import dev.ravel.legacy.input.Keyboard;
import dev.ravel.legacy.input.Mouse;
import dev.ravel.legacy.opengl.Display;
import dev.ravel.legacy.opengl.DisplayMode;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.Robot;
import java.awt.Toolkit;
import java.awt.event.InputEvent;
import java.awt.event.KeyEvent;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
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
            button(1);
            await(() -> onGame(() -> field(game, "f") == null));
            if (Boolean.getBoolean("ravel.legacy.acceptance")) {
                rendererAfterUnload = rendererMetric("currentGeneration");
                require(rendererAfterUnload > rendererAfterReload, "Renderer unload did not retire generation");
                rendererStaleRejected &= !rendererAccepts(rendererAfterReload);
            }
            snapshot("07-saved-menu");
            checkpoint("integrated-first.dat");
            reopen();
            checkpoint("integrated-reopened.dat");
            if (Boolean.getBoolean("ravel.legacy.acceptance")) rendererProof();
            Files.writeString(evidence.resolve("automation.complete"),
                    "seed=8675309\nactions=click-menu,type-worldname,type-seed,create,audio-playing,ticks,walk,look,wheel,resize,fullscreen,save-quit,reopen\n"
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
            if (Boolean.getBoolean("ravel.legacy.acceptance")) reloadResources();
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
        Files.writeString(evidence.resolve("renderer.json"),
                "{\"beforeReload\":" + rendererBeforeReload + ",\"afterReload\":" + rendererAfterReload
                + ",\"afterUnload\":" + rendererAfterUnload + ",\"reopened\":" + rendererReopened
                + ",\"finalUnload\":" + finalUnload + ",\"staleRejected\":" + rendererStaleRejected
                + ",\"ownerThread\":" + rendererMetric("ownerThread")
                + ",\"injectedStaleObservationRejected\":" + rendererInjectedStaleRejected
                + ",\"workerObserved\":" + worker + ",\"snapshot\":" + snapshots
                + ",\"seams\":{" + counts + "}}\n");
    }

    private String snapshotProof() throws Exception {
        long[] values = (long[]) rendererType().getMethod("snapshotMetrics").invoke(null);
        String[] names = {"captures", "totalBytes", "maxBytes", "totalNanos", "maxNanos",
                "mismatches", "cells", "rejected", "skipped", "borderCells", "lastThread",
                "lastGeneration", "nonAirCells", "adjacentVariation", "maxNativeCopyBytes",
                "maxNativeScratchBytes", "staleCaptureRejections", "workerCaptures",
                "oracleChecks", "oracleMismatches", "oracleBorderChecks",
                "oracleRetryCells", "oracleRetryCandidates"};
        require(values.length == names.length && values[0] > 0 && values[1] > 0
                && values[2] > 0 && values[3] > 0 && values[4] > 0 && values[5] == 0
                && values[6] == values[0] * 5832 && values[9] == values[0] * 1736
                && values[10] > 0 && values[11] > 0 && values[12] > 0 && values[13] > 0
                && values[14] == 29160 && values[15] > 0 && values[16] > 0 && values[17] > 0
                && values[18] == 125 && values[19] == 0 && values[20] == 98
                && values[22] >= 0 && values[21] >= values[22] && values[21] <= values[22] * 125,
                "Snapshot proof missing bounded vanilla nonair/border roundtrip or stale rejection");
        StringBuilder json = new StringBuilder("{\"seed\":8675309,\"edge\":18,"
                + "\"comparison\":\"decoded-vs-Java-copied-post-compile-region\","
                + "\"light\":\"raw-stored-sky-and-block-not-combined-mesh-light\","
                + "\"oracle\":\"coordinate-only-same-mappings\","
                + "\"modelKeys\":\"state-derived-not-baked-model-identities\","
                + "\"timingSampleLimit\":4096,\"maxConcurrentCaptures\":1,"
                + "\"javaBatchBytes\":29160,\"retainedStaleProofBytes\":29160,"
                + "\"maxJavaBatchPayloadBytes\":58320");
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
