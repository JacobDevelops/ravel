import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.block.Block;

/**
 * Drives the content adapter through the seam a real launch uses.
 *
 * <p>Nothing here calls Ravel. The registries are frozen the way the game
 * freezes them, the transformer's prologue is what reaches the adapter, and what
 * is printed is the stand-in registries read back afterwards — so a run that
 * registered nothing and a run that registered the wrong thing look different.
 */
public final class ContentMain {
    private ContentMain() {
    }

    public static void main(String[] args) {
        BuiltInRegistries.freeze();
        System.out.println(BuiltInRegistries.BLOCK.contents());
        System.out.println(BuiltInRegistries.ITEM.contents());
        // Every registered block's states have to have reached the mapper the
        // protocol encodes a block update through. A block that is in the
        // registry and in no mapper places and then disconnects the session.
        System.out.println("states=" + Block.BLOCK_STATE_REGISTRY.size()
                + " cached=" + Block.BLOCK_STATE_REGISTRY.cached());
        System.out.println("hooks=" + dev.ravel.jvm.Hook.nativeInvocations());
    }
}
