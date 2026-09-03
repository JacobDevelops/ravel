package net.minecraft.client.gui.components.events;

/**
 * A stand-in for where a screen's focus is held.
 *
 * <p>It is a class of its own here because it is one in the pinned jars: the
 * screens reach {@code getFocused} and {@code setFocused} through the supertype
 * and not through {@code Screen}, and a stand-in that declared them on the
 * screen would be a surface the generated stubs do not have.
 */
public abstract class AbstractContainerEventHandler implements GuiEventListener {

    private GuiEventListener focused;

    public GuiEventListener getFocused() {
        return focused;
    }

    public void setFocused(GuiEventListener focused) {
        this.focused = focused;
    }
}
