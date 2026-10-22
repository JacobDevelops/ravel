package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;

final class LegacyWorldDraw {
    record Entry(Object chunk, Object task, int[] origin, long generation, long key,
            boolean ready, int[] layers, long[] sorts, float[] sortCamera, int fluidLayers) { }
    private static final LinkedHashMap<Long, Entry> ENTRIES = new LinkedHashMap<>();
    static final int UNACKNOWLEDGED = 3, DRAIN_REFUSED = 4, FLUID = 8, SKIPPED = 9, REFUSED = 10,
            IN_FLIGHT = 11, OFFERED = 12;
    private static final boolean SELECTED = System.getProperty("ravel.renderer", "vanilla").equals("resident-world");
    private static final IdentityHashMap<Object, int[]> SECTIONS = new IdentityHashMap<>();
    private static final ArrayList<int[]> CHANGED = new ArrayList<>();
    private static final ArrayDeque<Object> RETRY = new ArrayDeque<>();
    private static final HashSet<Long> DRAINED = new HashSet<>();
    private static boolean reset;
    private static int retries;
    private static final int[] VBO = new int[4];
    private static final int[] EMPTY = new int[0];
    private static long configured;
    private static int distance;
    private static long admittedTranslucent;
    static synchronized long translucentAdmissions() { return admittedTranslucent; }
    private static final String[] AXES = {"getX", "getY", "getZ"};
    private static final java.util.HashMap<String, java.util.HashMap<String, Field>> FIELDS = new java.util.HashMap<>();
    private static final java.util.HashMap<String, java.util.HashMap<String, Method>> METHODS = new java.util.HashMap<>();
    private static Entry probe;
    private static int probeKind;
    private LegacyWorldDraw() { }
    static synchronized void status(Object task, int code, boolean settle, boolean retry) {
        if (!SELECTED) return;
        try {
            Object chunk = method("LEGACY_COMPILE_TASK", "getChunk").invoke(task);
            if (chunk != null) mark(chunk, code, settle, retry);
        } catch (ReflectiveOperationException failure) { throw new IllegalStateException("Resident section status failed", failure); }
    }
    private static void mark(Object chunk, int code, boolean settle, boolean retry) throws ReflectiveOperationException {
        int[] section = SECTIONS.get(chunk);
        if (section == null) {
            section = new int[7];
            Method buffer = method("LEGACY_RENDER_CHUNK", "getBuffer");
            for (int index = 0; index < 4; index++) {
                Object vertexBuffer = buffer.invoke(chunk, index);
                section[1 + index] = vertexBuffer == null ? 0 : field("LEGACY_VERTEX_BUFFER", "id").getInt(vertexBuffer);
            }
            SECTIONS.put(chunk, section);
        } else if (settle && section[0] != IN_FLIGHT) return;
        if (retry && section[6] == 0) RETRY.addLast(chunk);
        section[6] = retry ? 1 : section[6] == 0 ? 0 : 2;
        if (section[0] == code) return;
        section[0] = code;
        if (section[5] == 0) { section[5] = 1; CHANGED.add(section); }
    }
    static synchronized void frame() { retries = 2; }
    private static void retry() throws ReflectiveOperationException {
        while (retries > 0 && !RETRY.isEmpty()) {
            Object chunk = RETRY.pollFirst();
            int[] section = SECTIONS.get(chunk);
            if (section == null) continue;
            boolean wanted = section[6] == 1;
            section[6] = 0;
            if (!wanted) continue;
            method("LEGACY_RENDER_CHUNK", "setDirty").invoke(chunk, false);
            retries--;
        }
    }
    static synchronized void drained(long key) {
        Entry entry = ENTRIES.get(key);
        if (entry != null && !entry.ready()) DRAINED.add(key);
    }
    private static void flush() {
        if (!reset && CHANGED.isEmpty()) return;
        int[] pairs = new int[CHANGED.size() * 8];
        int at = 0;
        for (int[] section : CHANGED) {
            for (int index = 0; index < 4; index++) { pairs[at++] = section[1 + index]; pairs[at++] = section[0]; }
            section[5] = 0;
        }
        LegacyGeometryProof.worldStatus(pairs, reset);
        CHANGED.clear();
        reset = false;
    }
    private static int label(Entry entry, int reason) {
        if (reason < 0) return DRAINED.contains(entry.key()) ? DRAIN_REFUSED : UNACKNOWLEDGED;
        int[] section = SECTIONS.get(entry.chunk());
        int code = section == null ? 0 : section[0];
        return reason == 1 && (code == SKIPPED || code == REFUSED || code == FLUID) ? code : reason;
    }
    static synchronized int captureLimit(int bytes) {
        long diameter = distance * 2L + 1;
        long budget = Math.clamp(diameter * diameter * 262144L, 16L * 1024 * 1024, 256L * 1024 * 1024);
        return Math.clamp(budget / bytes, 1, 64);
    }
    static synchronized boolean configure(long generation) throws ReflectiveOperationException {
        if (!LegacyRenderer.accepts(generation)) return false;
        if (configured == generation) return true;
        int selected = viewDistance();
        if (Hook.ravelRendererBakedLease(generation, selected, 3)[0] != 1) return false;
        if (!LegacyGeometryProof.worldBudget(selected)) return false;
        distance = selected; configured = generation;
        return true;
    }
    static synchronized boolean distanceChanged() {
        if (configured == 0) return false;
        try { return distance != viewDistance(); }
        catch (ReflectiveOperationException failure) { throw new IllegalStateException("View distance unavailable", failure); }
    }
    private static int viewDistance() throws ReflectiveOperationException {
        Object client = method("LEGACY_CLIENT", "getInstance").invoke(null);
        Object options = field("LEGACY_CLIENT", "options").get(client);
        return field("LEGACY_GAME_OPTIONS", "viewDistance").getInt(options);
    }
    static synchronized void warm(int x, int z) throws ReflectiveOperationException {
        Object client = method("LEGACY_CLIENT", "getInstance").invoke(null);
        Object renderer = field("LEGACY_CLIENT", "worldRenderer").get(client);
        int radius = (distance + 1) * 16;
        method("LEGACY_WORLD_RENDERER", "setDirty")
                .invoke(renderer, x - radius, 0, z - radius, x + radius, 255, z + radius, true);
    }
    static synchronized void offer(Object chunk, Object task, int[] origin, long generation, long key,
            int[] emitted, int fluidLayers) {
        if (emitted[3] > 0) admittedTranslucent++;
        ENTRIES.values().removeIf(entry -> entry.chunk() == chunk || Arrays.equals(entry.origin(), origin));
        if (ENTRIES.size() == 4096) ENTRIES.remove(ENTRIES.keySet().iterator().next());
        if (!DRAINED.isEmpty()) DRAINED.retainAll(ENTRIES.keySet());
        ENTRIES.put(key, new Entry(chunk, task, origin.clone(), generation, key, false,
                Arrays.copyOf(emitted, 4), new long[3], new float[3], fluidLayers));
        if (!SELECTED) return;
        try { mark(chunk, OFFERED, false, false); }
        catch (ReflectiveOperationException failure) { throw new IllegalStateException("Resident section status failed", failure); }
    }
    static synchronized void acknowledge(long generation, long key) {
        Entry entry = ENTRIES.get(key);
        if (entry != null && entry.generation() == generation && LegacyWorldResort.acknowledge(key)) {
            entry = ENTRIES.get(key);
            ENTRIES.put(key, new Entry(entry.chunk(), entry.task(), entry.origin(), generation, key,
                    true, entry.layers(), entry.sorts(), entry.sortCamera(), entry.fluidLayers()));
        }
    }
    static synchronized void clear() {
        ENTRIES.clear(); probe = null; configured = 0; LegacyWorldResort.clear();
        SECTIONS.clear(); CHANGED.clear(); RETRY.clear(); DRAINED.clear(); reset = true;
    }
    static synchronized boolean ready(int x, int y, int z) {
        for (Entry entry : ENTRIES.values())
            if (entry.origin()[0] == x && entry.origin()[1] == y && entry.origin()[2] == z && current(entry)) return true;
        return false;
    }
    static synchronized long[] sorts(int x, int y, int z) {
        for (Entry entry : ENTRIES.values())
            if (entry.origin()[0] == x && entry.origin()[1] == y && entry.origin()[2] == z)
                return entry.sorts().clone();
        return new long[3];
    }
    static synchronized float[] sortCamera(int x, int y, int z) {
        for (Entry entry : ENTRIES.values())
            if (entry.origin()[0] == x && entry.origin()[1] == y && entry.origin()[2] == z)
                return entry.sortCamera().clone();
        return new float[3];
    }
    static synchronized boolean negative(int x, int y, int z, int kind) {
        if (kind != 1 && kind != 2) throw new IllegalArgumentException("Unknown world fallback proof");
        for (Entry entry : ENTRIES.values())
            if (entry.origin()[0] == x && entry.origin()[1] == y && entry.origin()[2] == z && current(entry)) {
                probe = entry; probeKind = kind; return true;
            }
        return false;
    }
    static synchronized Entry sorting(int[] origin) throws ReflectiveOperationException {
        for (Entry entry : ENTRIES.values())
            if (Arrays.equals(entry.origin(), origin) && LegacyRenderer.accepts(entry.generation())
                    && sameCompiled(entry)) return entry;
        return null;
    }
    static synchronized Entry resident(long key) { return ENTRIES.get(key); }
    static synchronized Object sortTask(Entry entry) throws ReflectiveOperationException {
        Object pending = field("LEGACY_RENDER_CHUNK", "pendingTask").get(entry.chunk());
        return pending == null ? entry.task() : pending;
    }
    static synchronized boolean sorted(long key, Object task) throws ReflectiveOperationException {
        Entry entry = ENTRIES.get(key);
        if (entry == null || !sameCompiled(entry)) return false;
        Object pending = field("LEGACY_RENDER_CHUNK", "pendingTask").get(entry.chunk());
        if (pending != null && pending != task) return false;
        ENTRIES.put(key, new Entry(entry.chunk(), task,
                entry.origin(), entry.generation(), key, entry.ready(), entry.layers(), entry.sorts(),
                entry.sortCamera(), entry.fluidLayers()));
        return true;
    }
    private static boolean sameCompiled(Entry entry) throws ReflectiveOperationException {
        Object compiled = method("LEGACY_COMPILE_TASK", "getCompiledChunk").invoke(entry.task());
        Object pending = field("LEGACY_RENDER_CHUNK", "pendingTask").get(entry.chunk());
        return compiled != null && method("LEGACY_RENDER_CHUNK", "getCompiledChunk").invoke(entry.chunk()) == compiled
                && (pending == null || method("LEGACY_COMPILE_TASK", "getCompiledChunk").invoke(pending) == compiled);
    }
    static synchronized void refuse(long key) { ENTRIES.remove(key); DRAINED.remove(key); }
    private static boolean current(Entry entry) { return reason(entry) == 0; }
    private static int reason(Entry entry) {
        if (!entry.ready()) return -1;
        if (!LegacyRenderer.accepts(entry.generation())
                || LegacyRuntime.resourceGeneration() != LegacyRuntime.completedResourceGeneration()) return 1;
        try {
            Object position = field("LEGACY_RENDER_CHUNK", "origin").get(entry.chunk());
            for (int axis = 0; axis < 3; axis++)
                if ((int) method("LEGACY_VEC3I", AXES[axis]).invoke(position) != entry.origin()[axis]) return 1;
            Object pending = field("LEGACY_RENDER_CHUNK", "pendingTask").get(entry.chunk());
            if (pending != null && pending != entry.task()
                    && (entry.layers()[3] != 0 || !sameCompiled(entry))) return 2;
            if ((boolean) method("LEGACY_RENDER_CHUNK", "isDirty").invoke(entry.chunk())) return 1;
            return method("LEGACY_RENDER_CHUNK", "getCompiledChunk").invoke(entry.chunk())
                    == method("LEGACY_COMPILE_TASK", "getCompiledChunk").invoke(entry.task())
                    && Hook.ravelRendererBakedLease(entry.generation(), entry.key(), 4)[0] == 1 ? 0 : 1;
        } catch (ReflectiveOperationException failure) { throw new IllegalStateException("Resident section readiness failed", failure); }
    }
    static synchronized void layer(Object value) {
        if (!System.getProperty("ravel.renderer", "vanilla").equals("resident-world")) return;
        int layer = value instanceof Enum<?> enumeration ? enumeration.ordinal() : -1;
        LegacyGeometryProof.worldLayer(layer, EMPTY, 0, 0);
        if (layer < 0 || layer >= 4) return;
        flush();
        try {
            if (layer == 0 && probe != null) {
                Object chunk = probe.chunk();
                if (probeKind == 2) method("LEGACY_RENDER_CHUNK", "startCompile").invoke(chunk);
                method("LEGACY_RENDER_CHUNK", "setDirty").invoke(chunk, true);
                probe = null;
            }
            if (LegacyBakedCapture.idle()) retry();
            Method buffer = method("LEGACY_RENDER_CHUNK", "getBuffer");
            Field id = field("LEGACY_VERTEX_BUFFER", "id");
            for (Entry entry : ENTRIES.values()) {
                int reason = reason(entry);
                for (int index = 0; index < 4; index++) {
                    Object vertexBuffer = buffer.invoke(entry.chunk(), index);
                    VBO[index] = vertexBuffer == null ? 0 : id.getInt(vertexBuffer);
                }
                if (reason == 0 && (entry.fluidLayers() & 1 << layer) != 0) reason = FLUID;
                if (reason == 0) LegacyGeometryProof.worldLayer(layer, VBO, entry.generation(), entry.key());
                else LegacyGeometryProof.worldFallback(layer, VBO[layer], label(entry, reason),
                        reason == FLUID ? 0 : Math.max(reason, 0));
            }
        } catch (ReflectiveOperationException failure) { throw new IllegalStateException("Resident world draw seam failed", failure); }
    }
    private static Field field(String owner, String name) throws ReflectiveOperationException {
        var members = FIELDS.computeIfAbsent(owner, ignored -> new java.util.HashMap<>());
        Field field = members.get(name);
        if (field == null) {
            field = Members.field(LegacyAccess.type(owner), LegacyNames.member(owner, name));
            members.put(name, field);
        }
        return field;
    }
    private static Method method(String owner, String name) throws ReflectiveOperationException {
        var members = METHODS.computeIfAbsent(owner, ignored -> new java.util.HashMap<>());
        Method method = members.get(name);
        if (method == null) { method = LegacyAccess.method(owner, name); members.put(name, method); }
        return method;
    }
}
