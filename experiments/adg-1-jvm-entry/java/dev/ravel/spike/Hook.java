package dev.ravel.spike;

/**
 * Callback surface a generated Ravel hook class would provide.
 *
 * <p>Both entry candidates inject a call to one of these methods at the head of
 * the pinned target method, so the only difference between them is how the
 * native implementation is bound and who owns process start.
 */
public final class Hook {

    /** Payload that must be rejected as invalid input. */
    public static final long INVALID_PAYLOAD = -1L;

    /** Payload that must make the callback itself fail. */
    public static final long FAILING_PAYLOAD = Long.MIN_VALUE;

    private static long javaInvocations;

    private Hook() {
    }

    /** Boundary callback bound through {@code System.loadLibrary} or RegisterNatives. */
    public static native long onNativeCallback(long payload);

    /** Reference callback with no boundary crossing, for transform-cost isolation. */
    public static long onJavaCallback(long payload) {
        if (payload == INVALID_PAYLOAD) {
            throw new IllegalArgumentException("invalid callback payload");
        }
        if (payload == FAILING_PAYLOAD) {
            throw new IllegalStateException("callback failed");
        }
        javaInvocations += payload;
        return javaInvocations;
    }

    /** Observed Java-callback invocations, kept live so the JIT cannot elide them. */
    public static long javaInvocations() {
        return javaInvocations;
    }
}
