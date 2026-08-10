package net.minecraft;

/**
 * Stands in for the class every Minecraft distribution carries.
 *
 * <p>Ravel asks for this one only when the client's entry point is absent, to
 * tell "this is not the client distribution" apart from "this process cannot see
 * Minecraft at all". Only the name matters, so this is empty.
 */
public final class SharedConstants {

    private SharedConstants() {
    }
}
