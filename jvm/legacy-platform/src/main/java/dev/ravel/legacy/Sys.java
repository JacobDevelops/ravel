package dev.ravel.legacy;

import java.awt.Desktop;
import java.io.IOException;
import java.net.URI;
import org.lwjgl.Version;

/** Java-owned services used by the pinned vanilla client, without LWJGL2 natives. */
public final class Sys {
    private static final long TIMER_ORIGIN = System.nanoTime();

    private Sys() {}

    public static long getTime() {
        return (System.nanoTime() - TIMER_ORIGIN) / 1_000_000L;
    }

    public static long getTimerResolution() {
        return 1_000L;
    }

    public static String getVersion() {
        return Version.getVersion();
    }

    public static boolean openURL(String url) {
        try {
            if (!Desktop.isDesktopSupported()) {
                return false;
            }
            Desktop desktop = Desktop.getDesktop();
            if (!desktop.isSupported(Desktop.Action.BROWSE)) {
                return false;
            }
            desktop.browse(URI.create(url));
            return true;
        } catch (IOException | IllegalArgumentException | UnsupportedOperationException
                | SecurityException failure) {
            return false;
        }
    }
}
