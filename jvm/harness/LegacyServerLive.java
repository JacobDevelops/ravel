package dev.ravel.jvm;

/** Patched-in acceptance only: actual command receiver and actual packet connection. */
public final class LegacyServerLive {
    private static boolean commands;

    private LegacyServerLive() {}

    public static boolean exercise(Object server) throws Exception {
        if (LegacyAccess.call("LEGACY_SERVER", "getWorld", server, 0) == null) return false;
        if (!commands) {
            LegacyCommandLive.exercise(server);
            commands = true;
        }
        return LegacyPacketLive.exerciseServer(server);
    }
}
