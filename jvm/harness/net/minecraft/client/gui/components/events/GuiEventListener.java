package net.minecraft.client.gui.components.events;

import net.minecraft.client.input.KeyEvent;

public interface GuiEventListener {

    /** What the wheel reaches. Declared here and defaulted, as the pinned jars do. */
    default boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
        return false;
    }

    /** What a keystroke reaches, in the shape the generated version declares. */
    default boolean keyPressed(KeyEvent event) {
        return false;
    }
}
