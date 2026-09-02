package dev.ravel.jvm.screen;

import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockBehaviour;

/**
 * Stands in for the sibling artifact's registration class.
 *
 * <p>The real one is generated per version and compiled against that version's
 * stubs, and the agent jar reaches it by name and argument count, which is
 * exactly why a stand-in proves the control flow: the same entry points under
 * the same names, over stand-in game classes the harness does have. That the
 * real class compiles against the pinned stubs is what
 * {@code jvm/build/build-screen.sh} proves, and it is a different claim.
 *
 * <p>This carries 26.1.2's shape, because the stand-in game beside it does: the
 * properties objects refuse to describe a value that was never given an id, so a
 * registration that only looked like one fails here.
 */
public final class RavelContent {

    private RavelContent() {
    }

    /** Registers one block with the behaviour the mod that claimed it described. */
    public static void registerBlock(
            String namespace,
            String path,
            float hardness,
            float resistance,
            boolean tool,
            boolean ticks) {
        Identifier id = Identifier.fromNamespaceAndPath(namespace, path);
        BlockBehaviour.Properties properties = BlockBehaviour.Properties.of()
                .setId(ResourceKey.create(Registries.BLOCK, id))
                .strength(hardness, resistance);
        if (tool) {
            properties = properties.requiresCorrectToolForDrops();
        }
        if (ticks) {
            properties = properties.randomTicks();
        }
        Registry.register(BuiltInRegistries.BLOCK, id, new Block(properties));
    }

    /** Registers one item, described the same way and for the same reason. */
    public static void registerItem(
            String namespace, String path, int stackSize, boolean fireResistant) {
        Identifier id = Identifier.fromNamespaceAndPath(namespace, path);
        Item.Properties properties = new Item.Properties()
                .setId(ResourceKey.create(Registries.ITEM, id))
                .stacksTo(stackSize);
        if (fireResistant) {
            properties = properties.fireResistant();
        }
        Registry.register(BuiltInRegistries.ITEM, id, new Item(properties));
    }
}
