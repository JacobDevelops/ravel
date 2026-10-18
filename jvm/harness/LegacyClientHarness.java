package dev.ravel.jvm;

import dev.ravel.legacy.launch.LegacyLoader;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;

/** Runs client-backed fixtures through the live platform's game-byte rewriting loader. */
public final class LegacyClientHarness {
    private LegacyClientHarness() { }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length < 4) {
            throw new IllegalArgumentException("expected classpath-file runtime|screens side and fixture arguments");
        }
        var urls = new ArrayList<URL>();
        for (String line : Files.readAllLines(Path.of(arguments[0]))) {
            if (!line.isBlank()) urls.add(Path.of(line).toUri().toURL());
        }
        int side = Integer.parseInt(arguments[2]);
        if (side != 0 && side != 1) throw new IllegalArgumentException("unknown side " + side);
        try (var loader = new LegacyLoader(urls.toArray(URL[]::new), ClassLoader.getSystemClassLoader())) {
            Thread.currentThread().setContextClassLoader(loader);
            String entry = side == 0 ? GameNames.CLIENT_ENTRY_POINT : GameNames.SERVER_ENTRY_POINT;
            Agent.entered(side, Class.forName(entry, false, loader));
            String[] fixture = Arrays.copyOfRange(arguments, 3, arguments.length);
            try {
                switch (arguments[1]) {
                    case "runtime" -> LegacyRuntimeMain.main(fixture);
                    case "screens" -> LegacyScreensMain.main(fixture);
                    default -> throw new IllegalArgumentException("unknown fixture " + arguments[1]);
                }
            } catch (Throwable failure) {
                failure.printStackTrace();
                throw failure;
            }
        }
    }
}
