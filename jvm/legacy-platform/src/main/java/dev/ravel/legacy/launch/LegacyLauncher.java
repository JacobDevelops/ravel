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
        if (Boolean.getBoolean("ravel.legacy.acceptance")) auditNativeAccess(loader, client);
        if (client) {
            Thread automation = new Thread(new ClientAutomation(loader, Path.of(args[2])), "ravel-legacy-automation");
            automation.setDaemon(true);
            automation.start();
        }
        String entry = client ? "net.minecraft.client.main.Main" : "net.minecraft.server.MinecraftServer";
        loader.loadClass(entry).getMethod("main", String[].class)
                .invoke(null, (Object) Arrays.copyOfRange(args, 3, args.length));
    }

    private static void auditNativeAccess(ClassLoader game, boolean client) throws ClassNotFoundException {
        Module hook = Class.forName("dev.ravel.jvm.Hook", false,
                LegacyLauncher.class.getClassLoader()).getModule();
        Module platform = LegacyLauncher.class.getModule();
        if (!"dev.ravel.agent".equals(hook.getName()) || !hook.isNativeAccessEnabled()
                || platform.isNativeAccessEnabled()) {
            throw new IllegalStateException("legacy JNI access is not isolated to the named agent");
        }
        if (client) {
            Module jna = Class.forName("com.sun.jna.Native", false, game).getModule();
            Module narrator = Class.forName("com.mojang.text2speech.Narrator", false, game).getModule();
            if (jna.isNativeAccessEnabled() || narrator.isNativeAccessEnabled()) {
                throw new IllegalStateException("unsupported narrator/JNA native access was granted");
            }
            System.out.println("[ravel-legacy] nativeAccess agent=dev.ravel.agent:true"
                    + " platform=false jna=false narrator=false");
        } else {
            System.out.println("[ravel-legacy] nativeAccess agent=dev.ravel.agent:true platform=false");
        }
    }
}
