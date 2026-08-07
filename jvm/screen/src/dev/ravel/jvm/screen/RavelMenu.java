package dev.ravel.jvm.screen;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Button$OnPress;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** Adds Ravel's mod list to the game's normal title-menu button column. */
public final class RavelMenu {

    private static final int BUTTON_WIDTH = 200;
    private static final int BUTTON_HEIGHT = 20;

    private RavelMenu() {
    }

    /** Adds one button immediately after the normal menu options. */
    public static void add(Object receiver, int bottom, int spacing) {
        Screen title = (Screen) receiver;
        Button button = Button.builder(Component.literal("Ravel Mods"), new Button$OnPress() {
                    @Override
                    public void onPress(Button pressed) {
                        Minecraft.getInstance().setScreen(open("openMods", null));
                    }
                })
                .bounds((title.width - BUTTON_WIDTH) / 2, bottom + spacing, BUTTON_WIDTH, BUTTON_HEIGHT)
                .build();
        add(title, button);
    }

    static Screen openSettings(String owner) {
        return open("open", owner);
    }

    static Screen openProfiling() {
        return open("openProfiling", null);
    }

    private static Screen open(String name, String owner) {
        try {
            Class<?> screens = Class.forName(
                    "dev.ravel.jvm.Screens", true, ClassLoader.getSystemClassLoader());
            Method open = owner == null
                    ? screens.getDeclaredMethod(name)
                    : screens.getDeclaredMethod(name, String.class);
            open.setAccessible(true);
            return (Screen) (owner == null ? open.invoke(null) : open.invoke(null, owner));
        } catch (InvocationTargetException failed) {
            Throwable cause = failed.getCause() == null ? failed : failed.getCause();
            throw new IllegalStateException("the Ravel screen could not be opened", cause);
        } catch (ReflectiveOperationException | RuntimeException unusable) {
            throw new IllegalStateException("the Ravel screen entry is unavailable", unusable);
        }
    }

    private static void add(Screen title, Button button) {
        try {
            Method add = Screen.class.getDeclaredMethod(
                    "addRenderableWidget", GuiEventListener.class);
            add.setAccessible(true);
            add.invoke(title, button);
        } catch (InvocationTargetException failed) {
            Throwable cause = failed.getCause() == null ? failed : failed.getCause();
            throw new IllegalStateException("the title screen refused Ravel's menu button", cause);
        } catch (ReflectiveOperationException | RuntimeException unusable) {
            throw new IllegalStateException("the title screen cannot accept Ravel's menu button", unusable);
        }
    }
}
