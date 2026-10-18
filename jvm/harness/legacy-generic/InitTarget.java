package fixture;

/** Loaded during initialize, after transformer registration but before entry body. */
public final class InitTarget {
    public static int bodies;
    private InitTarget() { }
    public static void run() { bodies++; }
}
