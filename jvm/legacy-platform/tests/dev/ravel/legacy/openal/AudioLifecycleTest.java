package dev.ravel.legacy.openal;

import java.util.concurrent.atomic.AtomicReference;
import org.lwjgl.BufferUtils;
import dev.ravel.legacy.LWJGLException;
import dev.ravel.legacy.Sys;

/** Standalone integration contract; requires a working OpenAL playback device. */
public final class AudioLifecycleTest {
    public static void main(String[] args) throws Exception {
        if (Sys.getTimerResolution() != 1_000L) {
            throw new AssertionError("timer ticks must be milliseconds");
        }
        long start = Sys.getTime();
        Thread.sleep(2);
        if (Sys.getTime() <= start) {
            throw new AssertionError("timer must advance monotonically");
        }
        for (int cycle = 0; cycle < 2; cycle++) {
            AL.create();
            try {
                if (!AL.isCreated()) {
                    throw new AssertionError("successful create must publish its context");
                }
                try {
                    AL.create();
                    throw new AssertionError("duplicate create must fail");
                } catch (LWJGLException expected) {
                    // The active context must survive a rejected duplicate create.
                }
                AtomicReference<Throwable> failure = new AtomicReference<>();
                Thread soundThread = new Thread(() -> {
                    try {
                        int source = AL10.alGenSources();
                        try {
                            AL10.alListener(AL10.AL_POSITION,
                                    BufferUtils.createFloatBuffer(3).put(new float[] {0, 0, 0}).flip());
                            AL10.alSource(source, AL10.AL_POSITION,
                                    BufferUtils.createFloatBuffer(3).put(new float[] {1, 2, 3}).flip());
                            AL10.alSourceStop(BufferUtils.createIntBuffer(1).put(0, source));
                            java.nio.FloatBuffer x = BufferUtils.createFloatBuffer(1);
                            java.nio.FloatBuffer y = BufferUtils.createFloatBuffer(1);
                            java.nio.FloatBuffer z = BufferUtils.createFloatBuffer(1);
                            AL10.alGetSource3f(source, AL10.AL_POSITION, x, y, z);
                            if (x.get(0) != 1 || y.get(0) != 2 || z.get(0) != 3) {
                                throw new AssertionError("source position must reach OpenAL");
                            }
                            if (AL10.alGetError() != AL10.AL_NO_ERROR) {
                                throw new AssertionError("legacy calls must succeed on sound thread");
                            }
                        } finally {
                            AL10.alDeleteSources(source);
                        }
                    } catch (Throwable error) {
                        failure.set(error);
                    }
                }, "legacy-audio-contract");
                soundThread.start();
                soundThread.join();
                if (failure.get() != null) {
                    throw new AssertionError("process-wide audio context unavailable", failure.get());
                }
            } finally {
                AL.destroy();
            }
            AL.destroy();
            if (AL.isCreated() || org.lwjgl.openal.ALC10.alcGetCurrentContext() != 0) {
                throw new AssertionError("destroy must release the current context");
            }
        }
    }
}
