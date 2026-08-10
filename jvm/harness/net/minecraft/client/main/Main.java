package net.minecraft.client.main;

/**
 * Stands in for the client distribution's entry point.
 *
 * <p>Ravel reads the running side by asking the JVM whether it can load this
 * class: the client distribution carries {@code net/minecraft/client/**} and the
 * dedicated server distribution does not. Only the name matters, so this is
 * empty — the probe loads the class and never calls anything on it.
 *
 * <p>Every harness case that runs premain therefore needs this on its class path
 * or Ravel refuses to name a side. That is the point: the case that leaves it out
 * is what proves the side is read rather than assumed.
 */
public final class Main {

    private Main() {
    }
}
