package dev.ravel.legacy.launch;

import dev.ravel.legacy.openal.AL;
import dev.ravel.legacy.openal.AL10;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;
import org.lwjgl.system.MemoryStack;

/** Plays a real PCM source through the game's process-wide OpenAL context. */
final class AudioAcceptance {
    private AudioAcceptance() {}

    static void play(Path evidence) throws Exception {
        synchronized (AL.class) {
            if (!AL.isCreated()) throw new IllegalStateException("OpenAL context was not created");
            int buffer = 0;
            int source = 0;
            try (MemoryStack stack = MemoryStack.stackPush()) {
                var samples = stack.mallocShort(22050);
                for (int sample = 0; sample < samples.capacity(); sample++) {
                    samples.put((short) (Math.sin(2 * Math.PI * 440 * sample / 22050) * 1000));
                }
                samples.flip();
                buffer = AL10.alGenBuffers();
                AL10.alBufferData(buffer, AL10.AL_FORMAT_MONO16, samples, 22050);
                source = AL10.alGenSources();
                AL10.alSourcei(source, AL10.AL_BUFFER, buffer);
                AL10.alSourcei(source, AL10.AL_LOOPING, AL10.AL_TRUE);
                AL10.alSourcei(source, AL10.AL_SOURCE_RELATIVE, AL10.AL_TRUE);
                AL10.alSourcePlay(source);
                long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
                while (AL10.alGetSourcei(source, AL10.AL_SOURCE_STATE) != AL10.AL_PLAYING) {
                    if (System.nanoTime() >= deadline) {
                        throw new IllegalStateException("OpenAL source never reached AL_PLAYING");
                    }
                    Thread.sleep(10);
                }
                Files.writeString(evidence.resolve("audio.txt"), "source=" + source
                        + " buffer=" + buffer + " state=AL_PLAYING\n");
            } finally {
                if (source != 0) {
                    AL10.alSourceStop(source);
                    AL10.alDeleteSources(source);
                }
                if (buffer != 0) AL10.alDeleteBuffers(buffer);
            }
        }
    }
}
