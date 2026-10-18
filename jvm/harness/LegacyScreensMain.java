package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.List;

/** Exercises generated legacy screen state against the installed native settings/mod surface. */
public final class LegacyScreensMain {
    private LegacyScreensMain() {}

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) throw new IllegalArgumentException("expected settings|settings-lock|screens and screen jar");
        Field installed = LegacyScreens.class.getDeclaredField("screens");
        installed.setAccessible(true);
        installed.set(null, LegacyScreens.loader(arguments[1], Agent.gameLoader()));
        if ("settings".equals(arguments[0])) settings();
        else if ("settings-lock".equals(arguments[0])) locks();
        else if ("screens".equals(arguments[0])) screens();
        else throw new IllegalArgumentException("unknown legacy screen section");
    }

    private static void locks() throws Exception {
        Object screen = LegacyScreens.openSettings("configuration", null);
        require(((List<?>) call(screen, "keys")).contains("dedicated.motd"), "actual server declaration supplies locked row");
        require(Boolean.FALSE.equals(call(screen, "editable", "dedicated.motd")), "real pack fixes the server setting");
        String original = (String) call(screen, "value", "dedicated.motd");
        require("Pack".equals(original), "displayed lock value comes from actual pack file");
        require(Boolean.FALSE.equals(call(screen, "edit", "dedicated.motd", "forbidden")), "Java rejects locked edit");
        require(original.equals(call(screen, "value", "dedicated.motd")), "locked editor value remains unchanged");
        String response = Hook.ravelSaveSettings("1:113:configuration1:114:dedicated.motd9:forbidden");
        require(!"1:0".equals(response) && response.contains("dedicated.motd"), "native save independently refuses locked edit");
        call(screen, "cancel");
        Object reopened = LegacyScreens.openSettings("configuration", null);
        require(original.equals(call(reopened, "value", "dedicated.motd")), "refused native lock edit cannot persist");
        call(reopened, "cancel");
        System.out.println("legacy-settings-lock=passed");
    }

    private static void settings() throws Exception {
        // This offline fixture skips client startup, which normally writes the save-admission marker.
        Files.writeString(Path.of("options.txt"), "version:1\n", StandardOpenOption.CREATE_NEW);
        Object screen = LegacyScreens.openSettings("configuration", null);
        require(screen != null, "real generated settings screen opens");
        require("configuration".equals(call(screen, "owner")), "native settings owner");
        Object keys = call(screen, "keys");
        require(keys instanceof List<?> list && list.contains("client.enabled") && list.contains("common.profile"),
                "native configuration declaration rows");
        String original = (String) call(screen, "value", "client.enabled");
        // Dispatch the generated game's actual mouse override, without an OpenGL context.
        Method left = screen.getClass().getSuperclass().getDeclaredMethod("left");
        left.setAccessible(true);
        int x = ((Integer) left.invoke(screen)) + 5;
        click(screen, x, 50);
        require(Boolean.toString(!Boolean.parseBoolean(original)).equals(call(screen, "value", "client.enabled")),
                "mapped legacy mouse input toggles the native row");
        call(screen, "reset");
        require(Boolean.TRUE.equals(call(screen, "edit", "client.enabled", "false")), "editable native row");
        require("false".equals(call(screen, "value", "client.enabled")), "Java edit state");
        call(screen, "reset");
        require(original.equals(call(screen, "value", "client.enabled")), "reset restores snapshot");
        call(screen, "edit", "client.enabled", "false");
        call(screen, "cancel");
        require(Boolean.TRUE.equals(call(screen, "finished")), "cancel ends session");
        Object reopened = LegacyScreens.openSettings("configuration", null);
        require(original.equals(call(reopened, "value", "client.enabled")), "cancel does not cross native save");
        call(reopened, "edit", "client.enabled", "not-a-boolean");
        require(Boolean.FALSE.equals(call(reopened, "save")), "native schema refuses invalid edit");
        require(!((String) call(reopened, "refused")).isEmpty(), "native refusal remains visible");
        require(Boolean.FALSE.equals(call(reopened, "finished")), "refusal keeps editor open");
        call(reopened, "edit", "client.enabled", "false");
        require(Boolean.TRUE.equals(call(reopened, "save")), "native valid edit saves: " + call(reopened, "refused"));
        Object saved = LegacyScreens.openSettings("configuration", null);
        require("false".equals(call(saved, "value", "client.enabled")), "reopen observes native saved value");
        // Restore the fixture through exactly the same public save path.
        call(saved, "edit", "client.enabled", original);
        require(Boolean.TRUE.equals(call(saved, "save")), "restore native fixture");
        System.out.println("legacy-settings=passed crossings=" + Hook.ravelSettingsCrossings());
    }

    private static void screens() throws Exception {
        Object mods = LegacyScreens.openMods();
        require(mods != null, "real generated mod menu opens");
        Object entries = call(mods, "entries");
        require(entries instanceof List<?> list && !list.isEmpty(), "actual native installed mod list");
        boolean configuration = false;
        for (Object entry : (List<?>) entries) {
            if ("configuration".equals(call(entry, "modId"))) {
                configuration = true;
                require(Boolean.TRUE.equals(call(entry, "configurable")), "native configurable flag");
            }
        }
        require(configuration, "configuration fixture appears in mod menu");
        Object settings = call(mods, "settings", "configuration");
        require("configuration".equals(call(settings, "owner")), "menu opens settings for selected native owner");
        Object refusal = LegacyScreens.openWorldRefused("legacy world admission refused");
        require("legacy world admission refused".equals(call(refusal, "diagnostic")), "native refusal presentation");
        require(Boolean.TRUE.equals(call(settings, "pausesGame")), "actual legacy screen pause behavior");
        System.out.println("legacy-screens=passed entries=" + ((List<?>) entries).size());
    }

    private static void click(Object screen, int x, int y) throws Exception {
        // The mods screen has only the mapped click override with this signature.
        // Settings also overrides release, so its parameter types alone are ambiguous.
        Class<?> mods = Class.forName(ScreenNames.RAVEL_MODS_SCREEN, false, screen.getClass().getClassLoader());
        Method click = null;
        for (Method method : mods.getDeclaredMethods()) {
            if (method.getReturnType() == void.class
                    && java.util.Arrays.equals(method.getParameterTypes(), new Class<?>[] {int.class, int.class, int.class})) {
                require(click == null, "generated legacy mouse click mapping is unambiguous");
                click = method;
            }
        }
        require(click != null, "generated legacy mouse override is absent");
        Method target = screen.getClass().getDeclaredMethod(click.getName(), int.class, int.class, int.class);
        require(target.getReturnType() == void.class, "generated legacy mouse click returns void");
        target.setAccessible(true);
        target.invoke(screen, x, y, 0);
    }

    private static Object call(Object receiver, String name, Object... arguments) throws Exception {
        Class<?>[] parameters = new Class<?>[arguments.length];
        for (int index = 0; index < arguments.length; index++) parameters[index] = String.class;
        return receiver.getClass().getMethod(name, parameters).invoke(receiver, arguments);
    }
    private static void require(boolean condition, String obligation) {
        if (!condition) throw new AssertionError(obligation);
    }
}
