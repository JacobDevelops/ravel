package net.minecraft.util.datafix;

import com.mojang.datafixers.DataFixer;

/** Stands in for the game's fixer holder, which the adapter only passes along. */
public final class DataFixers {

    private static final DataFixer FIXER = new DataFixer() {
    };

    private DataFixers() {
    }

    public static DataFixer getDataFixer() {
        return FIXER;
    }
}
