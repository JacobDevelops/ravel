import java.lang.reflect.Method;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;

public final class TitleMenuMain {
    public static void main(String[] args) throws ReflectiveOperationException {
        TitleScreen title = new TitleScreen();
        Method menu = TitleScreen.class.getDeclaredMethod(
                "createNormalMenuOptions", int.class, int.class);
        menu.setAccessible(true);
        int result = (int) menu.invoke(title, 100, 10);

        System.out.println("result=" + result);
        System.out.println("buttons=" + title.widgets().size());
        Button button = (Button) title.widgets().get(0);
        System.out.println("label=" + button.message().text());
        System.out.println("buttonY=" + button.y());
        button.press();
        Screen mods = Minecraft.getInstance().screen();
        System.out.println("screen=" + mods.getClass().getName());

        Method init = mods.getClass().getDeclaredMethod("init");
        init.setAccessible(true);
        init.invoke(mods);
        System.out.println("modWidgets=" + mods.widgets().size());
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof Button entry) {
                String label = entry.message().text();
                System.out.println("entry=" + label);
                if (label.equals("Configuration Settings")) {
                    entry.press();
                    System.out.println("settings="
                            + Minecraft.getInstance().screen().getClass().getName());
                    Minecraft.getInstance().setScreen(mods);
                } else if (label.equals("Profiling")) {
                    entry.press();
                    System.out.println("profiling="
                            + Minecraft.getInstance().screen().getClass().getName());
                    Minecraft.getInstance().setScreen(mods);
                }
            }
        }
    }
}
