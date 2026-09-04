// Prints the mod set the entry point installed, and nothing else.
//
// Which entry point loaded is the whole test: Ravel names the running side from
// the first distribution entry point the transformer sees, so a case entered at
// the client's is a client and one entered at the dedicated server's is a
// server. What the surface then says about each mod is the side declaration
// deciding, on the side it was told.
//
// A case that loads neither names no side and installs nothing, and the crossing
// answers nothing rather than a set composed for a guess. That is a different
// line, because a set is what `mods=` means.
public final class SideMain {
    public static void main(String[] args) {
        String surface = dev.ravel.jvm.Hook.ravelModSurface();
        System.out.println(surface == null ? "uninstalled" : "mods=" + surface);
    }
}
