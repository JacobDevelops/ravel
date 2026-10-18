package dev.ravel.legacy.opengl;

import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GLCapabilities;

public final class GLContext {
    private GLContext() {}
    private static final ThreadLocal<Snapshot> CURRENT = new ThreadLocal<>();
    private record Snapshot(GLCapabilities source, ContextCapabilities legacy) {}
    public static ContextCapabilities getCapabilities() {
        GLCapabilities source = GL.getCapabilities();
        Snapshot snapshot = CURRENT.get();
        if (snapshot == null || snapshot.source() != source) {
            snapshot = new Snapshot(source, new ContextCapabilities(source));
            CURRENT.set(snapshot);
        }
        return snapshot.legacy();
    }
}
