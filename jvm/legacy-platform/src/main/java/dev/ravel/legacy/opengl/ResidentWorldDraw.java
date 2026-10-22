package dev.ravel.legacy.opengl;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL15;

/** Bounded resident sections, submitted at vanilla's own VBO draw site. */
public final class ResidentWorldDraw {
    static final int BASELINE = 0, GL_EVICTED = 5, GL_OVER_BUDGET = 6, GL_STALE_KEY = 7,
            DROPPED = 12, UNTRACKED = 13, COUNT = 14, SHAPE = 15, REASONS = 16;
    private static final boolean SELECTED = System.getProperty("ravel.renderer", "vanilla").equals("resident-world");
    private static int layer = -1;
    private static final int[][] CANDIDATES = new int[4096][4];
    private static final int[] INDEX = new int[8192];
    private static int candidates;
    private static final long[] METRICS = new long[13];
    private static final long[] FALLBACKS = new long[4 * REASONS];
    private static int[] status = new int[0];
    private static boolean replacedThisFrame;
    private static int frameDraws;
    private static int frameVertices;
    private static int admittedDraws;
    private static int admittedVertices;
    private static final int[] COUNTS = new int[6];

    private ResidentWorldDraw() { }

    static void status(int[] pairs, boolean reset) {
        if (reset) java.util.Arrays.fill(status, 0);
        for (int index = 0; index < pairs.length; index += 2) {
            int buffer = pairs[index];
            if (buffer <= 0) continue;
            if (buffer >= status.length) status = java.util.Arrays.copyOf(status, Math.max(buffer + 1, status.length * 2));
            status[buffer] = pairs[index + 1];
        }
    }

    static void layer(int current, int source, int buffer, int count, int start) {
        if (current != layer || source == 0) {
            layer = current;
            candidates = 0;
            java.util.Arrays.fill(INDEX, 0);
        }
        if (source == 0 || candidates == CANDIDATES.length) return;
        int slot = slot(source);
        int index = INDEX[slot] == 0 ? candidates++ : INDEX[slot] - 1;
        INDEX[slot] = index + 1;
        int[] entry = CANDIDATES[index];
        entry[0] = source; entry[1] = buffer; entry[2] = count; entry[3] = start;
    }
    private static int slot(int source) {
        int slot = source * 0x9e3779b9 >>> 19;
        while (INDEX[slot] != 0 && CANDIDATES[INDEX[slot] - 1][0] != source)
            slot = (slot + 1) & (INDEX.length - 1);
        return slot;
    }

    static boolean draw(int mode, int first, int count) {
        if (CompositeFrame.replaying() || layer < 0 || count <= 0) return false;
        int bound = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        boolean shaped = layer < 4 && mode == GL11.GL_QUADS && first == 0;
        int index = shaped ? INDEX[slot(bound)] - 1 : -1;
        int[] candidate = index >= 0 ? CANDIDATES[index] : null;
        boolean admitted = candidate != null && candidate[1] != 0 && candidate[2] == count;
        if (candidate != null && candidate[2] < 0 && candidate[3] > 0 && !ProductionFrames.baseline())
            METRICS[10 + candidate[3]]++;
        if (admitted) {
            ProductionFrames.admission(admittedDraws, bound, candidate[1], count, candidate[3]);
            admittedDraws++; admittedVertices += count;
        }
        if (!admitted || ProductionFrames.baseline()) {
            METRICS[4 + layer]++;
            if (layer < 4) FALLBACKS[layer * REASONS + reason(candidate, admitted, shaped, bound)]++;
            if (!ProductionFrames.baseline()) COUNTS[layer == 3 ? 5 : 4]++;
            return false;
        }
        if (ProductionFrames.omitting()) return true;
        CompositeFrame.submit(mode, count, candidate[3], candidate[1], bound);
        ProductionFrames.replaced(layer, candidate[1]);
        METRICS[layer]++;
        COUNTS[layer]++;
        replacedThisFrame = true;
        frameDraws++;
        frameVertices += count;
        return true;
    }
    private static int reason(int[] candidate, boolean admitted, boolean shaped, int bound) {
        if (admitted) return BASELINE;
        if (!shaped) return SHAPE;
        if (candidate == null) return bound < status.length && status[bound] != 0 ? status[bound] : UNTRACKED;
        return candidate[2] < 0 ? -candidate[2] : COUNT;
    }

    static void presented(int width, int height) {
        if (!SELECTED) return;
        METRICS[8]++;
        if (replacedThisFrame) METRICS[9]++;
        METRICS[10] = Math.max(METRICS[10], frameDraws);
        admittedDraws = 0; admittedVertices = 0;
        replacedThisFrame = false; frameDraws = 0; frameVertices = 0;
        java.util.Arrays.fill(COUNTS, 0);
        layer = -1;
    }

    public static void worldRendered() {
        if (!SELECTED || CompositeFrame.replaying()) return;
        ProductionFrames.counts(COUNTS);
        ProductionFrames.presented(frameDraws, frameVertices, admittedDraws, admittedVertices,
                Display.getWidth(), Display.getHeight());
    }

    public static long[] metrics() { return METRICS.clone(); }
    public static long[] fallbacks() { return FALLBACKS.clone(); }
}
