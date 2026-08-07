import java.lang.reflect.Method;
import net.minecraft.client.gui.screens.TitleScreen;

public final class TitleMenuMissingMain {
    public static void main(String[] args) throws ReflectiveOperationException {
        TitleScreen title = new TitleScreen();
        Method menu = TitleScreen.class.getDeclaredMethod(
                "createNormalMenuOptions", int.class, int.class);
        menu.setAccessible(true);

        System.out.println("result=" + menu.invoke(title, 100, 10));
        System.out.println("buttons=" + title.widgets().size());
    }
}
