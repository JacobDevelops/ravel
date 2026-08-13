import net.minecraft.core.registries.BuiltInRegistries;

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
        System.out.println("hooks=" + dev.ravel.jvm.Hook.nativeInvocations());
    }
}
