package net.minecraft.client.gui.screens;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.components.events.AbstractContainerEventHandler;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.network.chat.Component;

public class Screen extends AbstractContainerEventHandler {
    public int width = 400;
    public int height = 300;
    protected final Font font = new Font();
    private final List<GuiEventListener> widgets = new ArrayList<>();

    protected Screen(Component title) {
    }

    protected void init() {
    }

    protected <T extends GuiEventListener> T addRenderableWidget(T widget) {
        widgets.add(widget);
        return widget;
    }

    protected void clearWidgets() {
        widgets.clear();
    }

    public void clearFocus() {
        setFocused(null);
    }

    /**
     * Focuses the first widget drawn, where the pinned class focuses the first
     * one a forward tab reaches and only when the last input was a keyboard.
     *
     * <p>The condition is dropped on purpose: a stand-in that only sometimes took
     * the focus away would only sometimes catch a screen that fails to put it
     * back.
     */
    protected void setInitialFocus() {
        setFocused(widgets.isEmpty() ? null : widgets.get(0));
    }

    /**
     * Lays the screen out again: clear the widgets, clear the focus, init, then
     * set the initial focus.
     *
     * <p>The pinned class clears and inits; whether it also sets the initial
     * focus on a rebuild is not something this repository can read. The stand-in
     * takes the focus away either way, which is the more hostile of the two.
     */
    protected void rebuildWidgets() {
        clearWidgets();
        clearFocus();
        init();
        setInitialFocus();
    }

    public List<GuiEventListener> widgets() {
        return List.copyOf(widgets);
    }

    /** Escape closes the screen and is used, as the pinned class has it. */
    @Override
    public boolean keyPressed(KeyEvent event) {
        if (event.key() == 256) {
            onClose();
            return true;
        }
        return super.keyPressed(event);
    }

    public void onClose() {
    }
}
