package dev.ravel.legacy;

/** Checked platform initialization failure retained for the legacy client ABI. */
public class LWJGLException extends Exception {
    private static final long serialVersionUID = 1L;

    public LWJGLException(String message) {
        super(message);
    }

    public LWJGLException(String message, Throwable cause) {
        super(message, cause);
    }

}
