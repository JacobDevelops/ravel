// Prints the mod set premain installed, and nothing else.
//
// The class path this runs on is the whole test: Ravel reads the running side by
// asking the JVM which Minecraft classes it can load, so a run whose class path
// carries the client's entry point is a client and one that carries only the
// class every distribution has is a dedicated server. What the surface then says
// about each mod is the side declaration deciding, on the side it was told.
public final class SideMain {
    public static void main(String[] args) {
        System.out.println("mods=" + dev.ravel.jvm.Hook.ravelModSurface());
    }
}
