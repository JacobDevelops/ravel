package dev.ravel.jvm;

/** Opt-in acceptance driver: actual owning-client-tick work, never shipped in the agent. */
public final class LegacyClientLive {
    private static boolean semantics;
    private static boolean packets;
    private static boolean complete;

    private LegacyClientLive() {}

    public static boolean exercise(Object game) throws Exception {
        if (complete) return true;
        if (LegacyAccess.call("LEGACY_CLIENT", "getNetworkHandler", game) == null) return false;
        if (!semantics) {
            LegacyRuntimeMain.exercise(game);
            semantics = true;
        }
        if (!packets) {
            packets = LegacyPacketLive.exercise(game);
            if (!packets) return false;
        }
        complete = LegacyScreensLive.exercise(game);
        if (complete) System.out.println("[ravel-test] legacy-client-live=passed");
        return complete;
    }

    public static boolean isComplete() {
        return complete;
    }
}
