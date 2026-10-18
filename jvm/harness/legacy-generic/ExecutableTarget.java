package fixture;

/** Compiled ordinary bytecode, rewritten and defined only by the verifier driver. */
public final class ExecutableTarget {
    public static int bodies;
    public final Object initialized;
    public long seed;
    public Object generator;
    public String settings;
    public static int staticNumber;
    public static Object staticFirst;
    public static Object staticSecond;

    public static void blockRegister(int number, Object key, Object value) {
        staticNumber = number;
        staticFirst = key;
        staticSecond = value;
        bodies++;
    }
    public static void itemRegister(int number, Object key, Object value) {
        staticNumber = number;
        staticFirst = key;
        staticSecond = value;
        bodies++;
    }
    public static Object staticState(int metadata) { bodies++; return staticFirst; }
    public static int staticMetadata(Object state) { bodies++; return 90004; }
    public static void recipe(Object key, Object value) {
        staticFirst = key;
        staticSecond = value;
        bodies++;
    }

    public ExecutableTarget(Object marker) {
        initialized = marker;
        bodies++;
    }

    public void open(String save, String display, long seed, Object generator, String settings) {
        this.seed = seed;
        this.generator = generator;
        this.settings = settings;
        bodies++;
    }

    public int itemMetadata() { bodies++; return 80002; }
    public int command(Object source, String text) { bodies++; return 23; }
    public boolean fire(Object source, float amount) { bodies++; return true; }
    public void throwing(String save, String display, long seed, Object generator, String settings) {
        bodies++;
        throw new IllegalStateException("fixture body failure");
    }
}
