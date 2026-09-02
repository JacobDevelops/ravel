package net.minecraft.client.gui.screens;

/**
 * Stands in for the screen type a refusal is shown as.
 *
 * <p>It carries the diagnostic so a run can be read back: what the harness has to
 * prove about a refusal is that the player was told, and a screen that held
 * nothing would let a refusal that showed a blank one pass.
 */
public class Screen {

    private final String diagnostic;

    public Screen(String diagnostic) {
        this.diagnostic = diagnostic;
    }

    @Override
    public String toString() {
        return diagnostic;
    }
}
