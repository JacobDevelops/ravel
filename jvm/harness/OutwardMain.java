import net.minecraft.client.Minecraft;

/**
 * Drives the outward adapter through the seam a real launch uses.
 *
 * <p>Nothing here calls Ravel. The client ticks the way the game ticks it, the
 * transformer's prologue is what reaches the adapter, and what is printed is the
 * stand-in sound manager read back afterwards — so a run that performed nothing
 * and a run that performed the wrong action look different.
 */
public final class OutwardMain {
    private OutwardMain() {
    }

    public static void main(String[] args) {
        Minecraft.getInstance().tick();
        System.out.println("played=" + Minecraft.getInstance().getSoundManager().contents());
        System.out.println("hooks=" + dev.ravel.jvm.Hook.nativeInvocations());
    }
}
