package dev.ravel.jvm.screen;

/**
 * Stands in for the sibling artifact's stored-data class.
 *
 * <p>The real one extends a game type and carries a codec, and the agent jar
 * reaches it by name through reflection, which is exactly why a stand-in proves
 * the control flow: the same three entry points under the same name, without a
 * game the harness does not have. That the real class compiles against the
 * pinned stubs is what {@code jvm/build/build-screen.sh} proves, and it is a
 * different claim from this one.
 *
 * <p>This is compiled into a jar named {@code ravel-screen.jar} beside the
 * harness agent, so a run without that jar is a run with the sibling artifact
 * genuinely absent rather than one with a flag set.
 */
public final class RavelWorldData {

    private RavelWorldData() {
    }

    /** The registration Minecraft would read and write Ravel's object through. */
    public static Object type() {
        return "ravel:mods";
    }

    /** Wraps replacement bytes as the object the storage would write. */
    public static Object of(byte[] aggregate) {
        return aggregate;
    }

    /** The stored aggregate, or {@code null} when there is no Ravel object. */
    public static byte[] bytes(Object stored) {
        return stored instanceof byte[] held ? held : null;
    }
}
