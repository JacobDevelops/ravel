package net.minecraft.world.item;

import net.minecraft.world.level.block.Block;

/**
 * Stands in for the item that places a block.
 *
 * <p>It holds the block, because that is the whole difference between it and a
 * plain item: a plain one has no use-on and right clicking it does nothing. The
 * harness reads the held block back, so an item registered plainly under a
 * block's identifier reads differently from one built from the block.
 */
public class BlockItem extends Item {

    private final Block block;

    public BlockItem(Block block, Item.Properties properties) {
        super(properties);
        this.block = block;
    }

    /** The block this item places, as the harness reads it back. */
    public Block getBlock() {
        return block;
    }
}
