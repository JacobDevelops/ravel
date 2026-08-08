package net.minecraft.server;

import java.util.function.BooleanSupplier;

/**
 * Stands in for the ticking server, carrying both members the inventory pins.
 *
 * <p>{@code tickServer} is the rewritten method and {@code tickCount} is the
 * field beside it, which vanilla advances in that same body.
 */
public final class MinecraftServer {

    private int tickCount;

    public void tickServer(BooleanSupplier hasTimeLeft) {
        if (hasTimeLeft.getAsBoolean()) {
            tickCount++;
        }
    }
}
