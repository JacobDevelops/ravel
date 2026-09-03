package net.minecraft.client.input;

/**
 * A stand-in for the keystroke the pinned version hands a screen.
 *
 * <p>The other pinned version passes the same three numbers as three arguments,
 * which is why the screen's keyboard override is the one generated source with a
 * per-version template rather than a per-version role binding.
 */
public record KeyEvent(int key, int scancode, int modifiers) {
}
