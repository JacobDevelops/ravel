package net.minecraft.server;

/**
 * Stands in for a distribution entry point that carries the name and not the method.
 *
 * <p>Ravel prepends its prologue to {@code main([Ljava/lang/String;)V} and to
 * nothing else. A class under the entry point's name that declares no such
 * method leaves Ravel installed and inert: the transformer has seen the class it
 * names the side from, and nothing runs. That is the fail-open answer the
 * shutdown report exists to refuse, so this class is what drives the report into
 * saying it.
 *
 * <p>The method below is deliberately the wrong shape rather than absent. A
 * class with no member at all would prove the same thing for a weaker reason,
 * and the descriptor is half of what the transformer matches on.
 */
public final class Main {

    private Main() {
    }

    /** The right name and the wrong descriptor, which is what makes this a miss. */
    public static void main(String argument) {
        throw new IllegalStateException(argument);
    }
}
