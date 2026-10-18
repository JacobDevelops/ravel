package dev.ravel.legacy.launch;

import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;

/** Parent class path contains only Ravel platform classes and LWJGL3. */
public final class LegacyLauncher {
    private LegacyLauncher() { }

    public static void main(String[] args) throws Exception {
        if (args.length < 3) throw new IllegalArgumentException("<client|server> <child-classpath-file> <evidence-dir> [game args]");
        var urls = new ArrayList<URL>();
        for (String line : Files.readAllLines(Path.of(args[1]))) {
            if (!line.isBlank()) urls.add(Path.of(line).toUri().toURL());
        }
        var loader = new LegacyLoader(urls.toArray(URL[]::new), LegacyLauncher.class.getClassLoader());
        Thread.currentThread().setContextClassLoader(loader);
        boolean client = args[0].equals("client");
        if (!client && !args[0].equals("server")) throw new IllegalArgumentException("Unknown side " + args[0]);
        if (client) {
            Thread automation = new Thread(new ClientAutomation(loader, Path.of(args[2])), "ravel-legacy-automation");
            automation.setDaemon(true);
            automation.start();
        }
        String entry = client ? "net.minecraft.client.main.Main" : "net.minecraft.server.MinecraftServer";
        loader.loadClass(entry).getMethod("main", String[].class)
                .invoke(null, (Object) Arrays.copyOfRange(args, 3, args.length));
    }
}
