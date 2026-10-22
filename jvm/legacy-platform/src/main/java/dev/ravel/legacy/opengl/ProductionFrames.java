package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.Instruction;
import java.lang.classfile.Opcode;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import org.lwjgl.opengl.ARBFramebufferObject;
import org.lwjgl.opengl.GL11;

/** Captures consecutive ordinary world frames before GUI composition; never invokes the renderer. */
public final class ProductionFrames {
    private static ByteBuffer raw;
    private static ByteBuffer arm;
    private static int stage = -1;
    private static int framebuffer;
    private static int width;
    private static int height;
    private static int warmup;
    private static byte[] completed;
    private static boolean bad;
    private static java.util.function.LongSupplier ticks;
    private static long capturedTick;
    private static long previousTick;
    private static int admitted;
    private static final int[] COUNTS = new int[5];
    private static final int[] ADMISSION = new int[4096 * 3 * 4];

    private ProductionFrames() { }

    public static byte[] transform(byte[] original, ClassLoader loader) {
        if (!System.getProperty("ravel.renderer", "vanilla").equals("resident-world")) return original;
        var file = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                ClassHierarchyResolver.ofResourceParsing(loader).cached()));
        var model = file.parse(original);
        if (!model.thisClass().asInternalName().equals("buq"))
            throw new IllegalArgumentException("Production capture requires buq");
        int[] returns = {0};
        byte[] result = file.transformClass(model, ClassTransform.transformingMethodBodies(
                method -> method.methodName().equalsString("b") && method.methodType().equalsString("(FJ)V"),
                (builder, element) -> {
                    if (element instanceof Instruction instruction && instruction.opcode() == Opcode.RETURN) {
                        builder.invokestatic(ClassDesc.of("dev.ravel.legacy.opengl.ResidentWorldDraw"),
                                "worldRendered", MethodTypeDesc.ofDescriptor("()V"));
                        returns[0]++;
                    }
                    builder.with(element);
                }));
        if (returns[0] == 0) throw new IllegalArgumentException("Missing normal world-render return");
        return result;
    }

    public static void begin(int scene, int designated, int buffer, int w, int h,
            java.util.function.LongSupplier clientTicks) {
        if (stage >= 0 || buffer <= 0 || w <= 0 || h <= 0 || w > 1920 || h > 1080)
            throw new IllegalStateException("Invalid production frame capture");
        width = w; height = h; framebuffer = buffer; warmup = 3; completed = null; bad = false;
        ticks = clientTicks;
        previousTick = ticks.getAsLong();
        if (previousTick <= 0) throw new IllegalStateException("Production frames require counted client ticks");
        int pixels = Math.multiplyExact(w, h);
        raw = ByteBuffer.allocate(32 + pixels * 24).order(ByteOrder.LITTLE_ENDIAN);
        raw.putInt(scene).putInt(designated).putInt(w).putInt(h).putInt(0).putInt(0).putInt(3).putInt(0);
        arm = ByteBuffer.allocateDirect(pixels * 8).order(ByteOrder.LITTLE_ENDIAN);
        stage = 0;
        admitted = 0;
        java.util.Arrays.fill(COUNTS, 0);
    }

    static boolean baseline() { return stage == 0; }
    static boolean omitting() { return stage == 2; }
    static void admission(int ordinal, int source, int target, int count, int offset) {
        if (stage < 0) return;
        int index = ordinal * 4;
        if (index < 0 || index + 3 >= ADMISSION.length) { bad = true; return; }
        if (stage == 0) {
            ADMISSION[index] = source; ADMISSION[index + 1] = target;
            ADMISSION[index + 2] = count; ADMISSION[index + 3] = offset;
        } else if (ADMISSION[index] != source || ADMISSION[index + 1] != target
                || ADMISSION[index + 2] != count || ADMISSION[index + 3] != offset) bad = true;
    }

    static void presented(int draws, int vertices, int admittedDraws, int admittedVertices, int w, int h) {
        if (stage < 0) return;
        if (w != width || h != height
                || GL11.glGetInteger(ARBFramebufferObject.GL_DRAW_FRAMEBUFFER_BINDING) != framebuffer) bad = true;
        long tick = ticks.getAsLong();
        long previous = previousTick;
        previousTick = tick;
        if (warmup-- > 0 || (stage == 0 && tick == previous)) return;
        if (stage == 0) capturedTick = tick;
        else if (tick != capturedTick) bad = true;
        int previousFramebuffer = GL11.glGetInteger(ARBFramebufferObject.GL_READ_FRAMEBUFFER_BINDING);
        try {
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_READ_FRAMEBUFFER, framebuffer);
            arm.clear();
            GL11.glReadPixels(0, 0, width, height, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE,
                    arm.slice(0, width * height * 4));
            GL11.glReadPixels(0, 0, width, height, GL11.GL_DEPTH_COMPONENT, GL11.GL_FLOAT,
                    arm.slice(width * height * 4, width * height * 4));
            raw.position(32 + stage * width * height * 8).put(arm);
            if (stage == 0) { raw.putInt(28, admittedVertices); admitted = admittedDraws; }
            if (stage == 1) raw.putInt(16, draws).putInt(20, vertices);
            if (admittedDraws != admitted || admitted <= 0 || admittedVertices != raw.getInt(28)) bad = true;
            stage++;
            if (stage == 3) {
                if (bad) raw.putInt(16, 0);
                completed = raw.array(); raw = null; arm = null; stage = -1;
            }
        } finally {
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_READ_FRAMEBUFFER, previousFramebuffer);
        }
    }

    public static long capturedTick() { return capturedTick; }
    static void counts(int[] counts) { if (stage == 1) System.arraycopy(counts, 0, COUNTS, 0, COUNTS.length); }
    public static int[] counts() { return COUNTS.clone(); }
    public static void cancel() { stage = -1; raw = null; arm = null; completed = null; }

    public static byte[] take() { byte[] result = completed; completed = null; return result; }
}
