import dev.ravel.jvm.Hook;
import dev.ravel.jvm.screen.RavelSettingsScreen;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;

public final class SettingsScreenMain {
    public static void main(String[] args) throws Exception {
        boolean refuseSave = args.length == 1 && "refuse-save".equals(args[0]);
        Screen opened = RavelSettingsScreen.open("configuration");
        RavelSettingsScreen screen = (RavelSettingsScreen) opened;
        Method init = RavelSettingsScreen.class.getDeclaredMethod("init");
        init.setAccessible(true);

        init.invoke(screen);
        List<GuiEventListener> first = screen.widgets();
        ((Button) first.get(first.size() - 3)).press();

        init.invoke(screen);
        List<GuiEventListener> resized = screen.widgets();
        ((Button) resized.get(resized.size() - 2)).press();
        if (refuseSave) {
            Files.writeString(Path.of("config"), "blocked");
        }
        screen.onClose();

        System.out.println("owner=" + screen.owner());
        if (refuseSave) {
            String refused = screen.refused();
            String workingDirectory = Path.of("").toAbsolutePath().toString();
            System.out.println("refusedPresent=" + !refused.isEmpty());
            System.out.println("refusedPrivatePath=" + refused.contains(workingDirectory));
        } else {
            System.out.println("refused=" + screen.refused());
        }
        System.out.println("crossings=" + Hook.ravelSettingsCrossings());
    }
}
