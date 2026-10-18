package dev.ravel.legacy.openal;

import java.nio.IntBuffer;
import dev.ravel.legacy.LWJGLException;
import org.lwjgl.openal.ALC;
import org.lwjgl.openal.ALC10;
import org.lwjgl.openal.ALCCapabilities;

/** The process-wide OpenAL lifecycle expected by the pinned Paulscode backend. */
public final class AL {
    private static long device;
    private static long context;

    private AL() {}

    public static synchronized boolean isCreated() {
        return context != 0;
    }

    public static synchronized void create() throws LWJGLException {
        if (context != 0 || device != 0) {
            throw new LWJGLException("OpenAL resources are already open");
        }
        long openedDevice = 0;
        long createdContext = 0;
        boolean current = false;
        try {
            openedDevice = ALC10.alcOpenDevice((java.nio.ByteBuffer) null);
            if (openedDevice == 0) {
                throw new LWJGLException("Cannot open the default OpenAL playback device");
            }
            ALCCapabilities capabilities = ALC.createCapabilities(openedDevice);
            createdContext = ALC10.alcCreateContext(openedDevice, (IntBuffer) null);
            if (createdContext == 0) {
                throw new LWJGLException("Cannot create OpenAL context: ALC error "
                        + ALC10.alcGetError(openedDevice));
            }
            if (!ALC10.alcMakeContextCurrent(createdContext)) {
                throw new LWJGLException("Cannot make OpenAL context current: ALC error "
                        + ALC10.alcGetError(openedDevice));
            }
            current = true;
            // Paulscode creates and uses sources on multiple threads. Do not use
            // ALC_EXT_thread_local_context: its context must be process-wide.
            org.lwjgl.openal.AL.setCurrentProcess(
                    org.lwjgl.openal.AL.createCapabilities(capabilities));
            device = openedDevice;
            context = createdContext;
        } catch (LWJGLException | RuntimeException | LinkageError failure) {
            try {
                if (current) {
                    org.lwjgl.openal.AL.setCurrentProcess(null);
                    if (!ALC10.alcMakeContextCurrent(0)) {
                        throw new IllegalStateException("Cannot detach failed OpenAL context");
                    }
                }
                if (createdContext != 0) {
                    ALC10.alcDestroyContext(createdContext);
                }
                if (openedDevice != 0 && !ALC10.alcCloseDevice(openedDevice)) {
                    throw new IllegalStateException("Cannot close failed OpenAL device");
                }
            } catch (RuntimeException | LinkageError cleanupFailure) {
                failure.addSuppressed(cleanupFailure);
            }
            if (failure instanceof LWJGLException exception) {
                throw exception;
            }
            throw new LWJGLException("Cannot initialize OpenAL", failure);
        }
    }

    public static synchronized void destroy() {
        if (context != 0) {
            if (!ALC10.alcMakeContextCurrent(0)) {
                throw new IllegalStateException("Cannot detach OpenAL context");
            }
            org.lwjgl.openal.AL.setCurrentProcess(null);
            ALC10.alcDestroyContext(context);
            context = 0;
        }
        if (device != 0) {
            if (!ALC10.alcCloseDevice(device)) {
                throw new IllegalStateException("Cannot close OpenAL playback device");
            }
            device = 0;
        }
    }
}
