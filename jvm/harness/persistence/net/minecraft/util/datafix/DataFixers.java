package net.minecraft.util.datafix;

/** Stands in for the game's fixer holder, which the adapter only passes along. */
public final class DataFixers {

    private static final Object FIXER = new Object();

    private DataFixers() {
    }

    public static Object getDataFixer() {
        return FIXER;
    }
}
