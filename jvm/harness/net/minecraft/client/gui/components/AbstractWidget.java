package net.minecraft.client.gui.components;

import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.network.chat.Component;

public class AbstractWidget implements GuiEventListener {
    private Component message;

    protected AbstractWidget(Component message) {
        this.message = message;
    }

    public void setMessage(Component message) {
        this.message = message;
    }

    public Component message() {
        return message;
    }
}
