import java.lang.reflect.Method;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
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

        // The window this screen is laid out in. A player's GUI scale decides it,
        // and it is what the row budget is measured against, so the harness names
        // one rather than taking the stub's default for every run.
        if (args.length > 0) {
            String[] size = args[0].split("x");
            mods.width = Integer.parseInt(size[0]);
            mods.height = Integer.parseInt(size[1]);
        }

        Method init = mods.getClass().getDeclaredMethod("init");
        init.setAccessible(true);
        init.invoke(mods);
        System.out.println("modWidgets=" + mods.widgets().size());
        Method notices = mods.getClass().getMethod("notices");
        System.out.println("notices=" + ((List<?>) notices.invoke(mods)).size());
        // The drawn lines rather than the model, because the finding launch 14
        // recorded was that a notice existed and no widget carried it. Each line
        // states whether it renders whole, because a notice drawn on one box too
        // narrow for it shows only its tail and is no more readable than none.
        int bottom = 0;
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof EditBox box) {
                bottom = Math.max(bottom, box.y() + box.height());
            } else if (widget instanceof Button drawn) {
                bottom = Math.max(bottom, drawn.y() + drawn.height());
            }
        }
        System.out.println("within=" + (bottom <= mods.height));

        List<?> rows = (List<?>) mods.getClass().getMethod("noticeRows").invoke(mods);
        System.out.println("noticeRows=" + rows.size());
        // Every drawn line, tagged by which group it came out of. The screen
        // divides the window between the notices, the mod rows and the settings
        // entries, so a run that only counted widgets could not tell a division
        // that starved a group from one that did not.
        boolean summary = true;
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof EditBox box) {
                if (summary) {
                    System.out.println("summary=" + box.fits() + " " + box.getValue());
                    summary = false;
                } else if (rows.contains(box.getValue())) {
                    System.out.println("noticeRow=" + box.fits() + " " + box.getValue());
                } else {
                    System.out.println("row=" + box.fits() + " " + box.getValue());
                }
            }
        }
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof Button entry) {
                String label = entry.message().text();
                System.out.println("entry=" + label);
                if (label.startsWith("Settings: ")) {
                    entry.press();
                    Screen settings = Minecraft.getInstance().screen();
                    System.out.println("settings=" + settings.getClass().getName());
                    // Reflection because the screen jar is loaded by Ravel's own
                    // loader, which this class's does not see.
                    Method owner = settings.getClass().getMethod("owner");
                    System.out.println("settingsOwner=" + owner.invoke(settings));
                    Method settingsInit = settings.getClass().getDeclaredMethod("init");
                    settingsInit.setAccessible(true);
                    settingsInit.invoke(settings);
                    System.out.println("settingsRows=" + (settings.widgets().size() - 1));
                    Minecraft.getInstance().setScreen(mods);
                } else if (label.equals("Profiling")) {
                    entry.press();
                    System.out.println("profiling="
                            + Minecraft.getInstance().screen().getClass().getName());
                    Minecraft.getInstance().setScreen(mods);
                }
            }
        }

        // Two payloads the screen must refuse whole. This crossing has no
        // hostile-payload seam of its own — the surface comes from the native and
        // nothing between them can rewrite it — so the refusing branches are
        // reached the way this class reaches `init`, through the same loader.
        // The third payload differs from the first only in the notice count and
        // must read, so a screen that refused everything would fail here rather
        // than pass twice.
        Method parse = mods.getClass().getDeclaredMethod("parse", String.class);
        parse.setAccessible(true);
        String head = field("3") + field("client") + field("0") + field("0");
        System.out.println("hostile=negativeNotices:" + parse.invoke(mods, head + field("-1")));
        System.out.println(
                "hostile=shortNoticeBlock:"
                        + parse.invoke(mods, head + field("2") + field("the only one")));
        System.out.println("hostile=wellFormed:" + parse.invoke(mods, head + field("0")));

        // A launch with no mod installed and something to tell the player about.
        // The summary has to keep counting the notice: a screen that stopped at
        // "No mods are installed." would draw a notice above a line saying
        // nothing needed attention. A fresh screen because `parse` adds to what
        // the instance above already read.
        Screen empty = (Screen) mods.getClass().getMethod("open").invoke(null);
        Method emptyParse = empty.getClass().getDeclaredMethod("parse", String.class);
        emptyParse.setAccessible(true);
        System.out.println("emptyRead=" + emptyParse.invoke(
                empty, head + field("1") + field("the pack is off")));
        // What `init` records when the payload reads, set here because this
        // screen was fed one directly rather than through the hook.
        java.lang.reflect.Field emptyRead = empty.getClass().getDeclaredField("read");
        emptyRead.setAccessible(true);
        emptyRead.setBoolean(empty, true);
        Method summaryLine = empty.getClass()
                .getDeclaredMethod("summary", int.class, int.class, int.class);
        summaryLine.setAccessible(true);
        System.out.println("emptySummary=" + summaryLine.invoke(empty, 1, 0, 0));
    }

    /** One field on the screen's wire, which states the length it is read with. */
    private static String field(String value) {
        return value.length() + ":" + value;
    }
}
