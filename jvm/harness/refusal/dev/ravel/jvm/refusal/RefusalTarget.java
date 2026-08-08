package dev.ravel.jvm.refusal;

/**
 * A void method a prologue is allowed to refuse.
 *
 * <p>It counts its own body rather than returning anything, because that is the
 * only thing the refusal changes: the call still happens and the caller still
 * proceeds, and what a refused call does not do is run this.
 */
public final class RefusalTarget {

    private static int bodies;

    private RefusalTarget() {
    }

    /** The body a refusing prologue must skip. */
    public static void record() {
        bodies++;
    }

    /** How many times the body ran. */
    public static int bodies() {
        return bodies;
    }
}
