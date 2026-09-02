package net.minecraft.client.sounds;

/**
 * Stands in for the sound engine only so its nested result type has the binary
 * name the reviewed surface declares {@code SoundManager.play} to answer.
 *
 * <p>The constants are the pinned 26.1.2 ones. Nothing on this side reads the
 * answer — a mod asked for a sound and the game decides what became of it — but
 * a stand-in returning {@code void} would let an adapter pass here against a
 * version whose method does not.
 */
public final class SoundEngine {

    private SoundEngine() {
    }

    /** What the game made of one play request. */
    public enum PlayResult {
        STARTED,
        STARTED_SILENTLY,
        NOT_STARTED
    }
}
