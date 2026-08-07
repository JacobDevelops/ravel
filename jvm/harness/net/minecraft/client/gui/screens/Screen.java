package net.minecraft.client.gui.screens;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.network.chat.Component;

public class Screen implements GuiEventListener {
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

    public List<GuiEventListener> widgets() {
        return List.copyOf(widgets);
    }

    public void onClose() {
    }
}
