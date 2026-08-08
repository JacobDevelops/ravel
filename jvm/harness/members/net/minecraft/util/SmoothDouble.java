package net.minecraft.util;

/**
 * Stands in for the input smoother, carrying both members the inventory pins.
 *
 * <p>{@code getNewDeltaValue} is the rewritten method and {@code lastAmount} is
 * the field beside it. The field is what the method leaves behind, which is why
 * reading it after the rewritten call says something: a rewrite that damaged the
 * class would not have produced this value from this field.
 */
public final class SmoothDouble {

    private double lastAmount;

    public double getNewDeltaValue(double target, double scale) {
        lastAmount = target * scale;
        return lastAmount;
    }
}
