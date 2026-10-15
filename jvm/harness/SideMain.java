// Reports the side-selected mod set, or its absence when no entry point loaded.
public final class SideMain {
    public static void main(String[] args) {
        String surface = dev.ravel.jvm.Hook.ravelModSurface();
        System.out.println(surface == null ? "uninstalled" : "mods=" + surface);
    }
}
