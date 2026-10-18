package dev.ravel.legacy.openal;

import java.nio.FloatBuffer;
import java.nio.IntBuffer;

/** Legacy spellings; unchanged entry points resolve to the LWJGL3 superclass. */
public final class AL10 extends org.lwjgl.openal.AL10 {
    private AL10() {}

    public static void alListener(int parameter, FloatBuffer values) {
        alListenerfv(parameter, values);
    }

    public static void alSource(int source, int parameter, FloatBuffer values) {
        alSourcefv(source, parameter, values);
    }

    public static void alSourceStop(IntBuffer sources) {
        alSourceStopv(sources);
    }
}
