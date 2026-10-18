package dev.ravel.jvm;

/** Isolated fixture surface: production 1.12.2 has no refusal-capable hook. */
public final class Hook {
    public static int calls;
    public static boolean verdict;
    public static int initializations;
    public static int side = -1;
    public record Observation(String id, Object receiver, Object first, Object second,
            int number, Object result, int resultNumber) {}
    public static final java.util.List<Observation> entries = new java.util.ArrayList<>();
    public static final java.util.List<Observation> exits = new java.util.ArrayList<>();
    public static Integer commandResult;
    public static Object commandReceiver;
    public static Object commandSource;
    public static String commandText;
    public static void resetObservations() {
        entries.clear();
        exits.clear();
        calls = 0;
        verdict = false;
        commandResult = null;
        commandReceiver = null;
        commandSource = null;
        commandText = null;
    }
    public static void legacyEnter(String id, Object receiver, Object first, Object second, int number) {
        entries.add(new Observation(id, receiver, first, second, number, null, 0));
    }
    public static void legacyExit(String id, Object receiver, Object first, Object second,
            int number, Object result, int resultNumber) {
        exits.add(new Observation(id, receiver, first, second, number, result, resultNumber));
    }
    public static Integer legacyCommand(Object receiver, Object source, String text) {
        commandReceiver = receiver;
        commandSource = source;
        commandText = text;
        return commandResult;
    }
    public static Object refusalReceiver;
    public static Object refusalSource;
    public static long nativeSeed;
    public static Object nativeGenerator;
    public static String nativeSettings;
    public static boolean legacyRefuse(String id, Object receiver, Object source) {
        refusalReceiver = receiver;
        refusalSource = source;
        return verdict;
    }
    public static void legacyCommandInstall(Object server) { calls++; }
    public static void legacyWorldOpen(String save, String display, long seed, Object generator, String settings) {
        calls++;
        nativeSeed = seed;
        nativeGenerator = generator;
        nativeSettings = settings;
    }
    public static int nativeNumber;
    public static Object nativeFirst;
    public static Object nativeSecond;
    public static void legacyBlockRegister(int number, Object key, Object value) {
        calls++; nativeNumber = number; nativeFirst = key; nativeSecond = value;
    }
    public static void legacyItemRegister(int number, Object key, Object value) {
        calls++; nativeNumber = number; nativeFirst = key; nativeSecond = value;
    }
    public static void legacyRecipeRegister(Object key, Object value) {
        calls++; nativeFirst = key; nativeSecond = value;
    }
    public static void legacyBlockStateFromMetadata(int metadata) { calls++; nativeNumber = metadata; }
    public static void legacyBlockMetadataFromState(Object state) { calls++; nativeFirst = state; }
    public static int paletteCalls;
    public static int paletteInput;
    public static void legacyGlobalPaletteSize(int bits) { calls++; }
    public static int legacyPaletteSize(int size) {
        paletteCalls++;
        paletteInput = size;
        return Math.max(size, 8192);
    }
    public static boolean legacyPaletteSizeCall(String owner, String name, String descriptor) {
        return owner.equals("dev/ravel/jvm/PaletteFixture$Mapper") && name.equals("size") && descriptor.equals("()I");
    }
    public static boolean legacyPaletteField(String owner, String name, String descriptor) {
        return owner.equals("dev/ravel/jvm/PaletteFixture") && name.equals("STATES")
                && descriptor.equals("Ldev/ravel/jvm/PaletteFixture$Mapper;");
    }
    public static void legacyItemMetadata() { calls++; }
    public static void legacyCommandExecute(Object source, String text) { calls++; }
    public static void legacyItemFireDamage(Object source, float amount) { calls++; }
    public static int observations;
    public static void legacyObserve(String id) { observations++; }
    public static final HookTarget[] TARGETS = {
        new HookTarget("fixture/InitTarget", "run", "()V", true, "probe", "()V", false)
    };
    public static void initialize() { }
    public static long nativeInvocations() { return calls; }
    public static void ravelInitialize(int named, boolean profiling) {
        initializations++;
        side = named;
        try {
            Class.forName("fixture.InitTarget", true, ClassLoader.getSystemClassLoader())
                    .getMethod("run").invoke(null);
        } catch (ReflectiveOperationException failure) {
            throw new AssertionError("cannot exercise initialization-loaded target", failure);
        }
    }
    private Hook() { }
    public static boolean decide() {
        calls++;
        return verdict;
    }
    public static void probe() { calls++; }
    public static void probe(boolean ignored) { calls++; }
}
