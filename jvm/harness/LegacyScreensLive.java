package dev.ravel.jvm;

/** Invoke only on the actual client thread; returns true after both real screens drew. */
public final class LegacyScreensLive {
    private LegacyScreensLive() {}

    public static boolean exercise(Object game) {
        if (!LegacyScreens.isInstalled()) throw new AssertionError("legacy screen jar is not installed");
        return LegacyScreens.exercise(game);
    }
}
