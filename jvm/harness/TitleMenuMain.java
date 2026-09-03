import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.input.KeyEvent;

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
        // and it is what the window's row count is measured against, so the harness names
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
        // states whether it renders whole, because a notice drawn on one row too
        // narrow for it is clipped and is no more readable than none.
        System.out.println("within=" + within(mods));
        // Every drawn line starts at the left edge of the box it is drawn in.
        // One pinned version centres a label in its box and the other draws it
        // at the edge, so a row drawn in a box wider than its text is a row the
        // two versions put in different places.
        System.out.println("aligned=" + aligned(mods));

        List<?> rows = (List<?>) mods.getClass().getMethod("noticeRows").invoke(mods);
        System.out.println("noticeRows=" + rows.size());
        // Every drawn line, tagged by which group it came out of, at the top of
        // the list. `walk` below is what proves the rest of the list is reachable;
        // this is what says the window opens on the rows it should.
        boolean summary = true;
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof StringWidget line) {
                if (summary) {
                    System.out.println("summary=" + line.fits() + " " + line.text());
                    summary = false;
                } else if (rows.contains(line.text())) {
                    System.out.println("noticeRow=" + line.fits() + " " + line.text());
                } else {
                    System.out.println("row=" + line.fits() + " " + line.text());
                }
            }
        }

        // The scrolling contract, and the offset is left back at the top so the
        // sections below see the window this run opened on.
        walk(mods);
        keys(mods);

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
                    System.out.println("settingsDone=" + closes(settings)
                            + " saves=" + Class.forName("dev.ravel.jvm.Hook")
                                    .getMethod("ravelSettingsCrossings").invoke(null));
                    Minecraft.getInstance().setScreen(mods);
                } else if (label.equals("Profiling")) {
                    entry.press();
                    Screen profiling = Minecraft.getInstance().screen();
                    System.out.println("profiling=" + profiling.getClass().getName());
                    Method profilingInit = profiling.getClass().getDeclaredMethod("init");
                    profilingInit.setAccessible(true);
                    profilingInit.invoke(profiling);
                    System.out.println("profilingDone=" + closes(profiling));
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
        Method summaryLine = empty.getClass().getDeclaredMethod("summary");
        summaryLine.setAccessible(true);
        System.out.println("emptySummary=" + summaryLine.invoke(empty));

        // A failed mod, which the composed set has none of: every mod in it
        // loads, and the failing branch of `Entry.lines` is therefore reached
        // through the same parse path the hostile payloads above use. That line
        // is the longest this screen composes — the kind, the surface it failed
        // at, the chain it was pulled in through and the detail, all on one row
        // — and it is the one a window has to split rather than clip.
        Screen failing = (Screen) mods.getClass().getMethod("open").invoke(null);
        failing.width = mods.width;
        failing.height = mods.height;
        Method failingParse = failing.getClass().getDeclaredMethod("parse", String.class);
        failingParse.setAccessible(true);
        System.out.println("failedRead=" + failingParse.invoke(failing, failure()));
        // The read this payload stands in for, and the guard that stops `init`
        // from making the crossing over it.
        set(failing, "read", true);
        set(failing, "loaded", true);
        Method failingInit = failing.getClass().getDeclaredMethod("init");
        failingInit.setAccessible(true);
        failingInit.invoke(failing);
        boolean failedSummary = true;
        for (GuiEventListener widget : failing.widgets()) {
            if (widget instanceof StringWidget line) {
                System.out.println((failedSummary ? "failedSummary=" : "failedRow=")
                        + line.fits() + " " + line.text());
                failedSummary = false;
            }
        }
    }

    /**
     * A payload whose one mod failed, composing a line no window can draw whole.
     *
     * <p>The detail is long on purpose. A mod row was the one group this screen
     * never split, so a failure's sentence was drawn on one row and clipped, and
     * a diagnostic whose detail is off the end of the row is the failure mode the
     * notice lines were split to avoid.
     */
    private static String failure() {
        return field("3") + field("client") + field("1") + field("1")
                + field("1") + field("brokenmod") + field("failed") + field("required")
                + field("true") + field("true") + field("false") + field("1")
                + field("a required registry claim was refused")
                + field("registry") + field("worldgen/biome")
                + field("registration") + field("2.1.0")
                + field("2") + field("brokenmod") + field("otherpack")
                + field("the claim was already held by another mod and this one asked for it "
                        + "again, so neither could be registered")
                + field("0");
    }

    /**
     * Moves the window with the keyboard, and keeps the focus while it moves.
     *
     * <p>What this proves is the half of the scrolling contract the wheel does
     * not reach. Up and Down move one row, the page keys move one window, a key
     * this screen has no use for is left to the game, and a window with nothing
     * to scroll refuses all four. Then the part a layout is most likely to break:
     * the game's own layout clears the focus and sets the initial one, so a
     * player who tabbed onto a settings entry and pressed a key would be moved
     * off it unless the screen puts the focus back. {@code drawn} says whether
     * the window it landed on still draws that entry, and {@code kept} has to
     * agree with it — the focus is restored where the entry survived and nowhere
     * else.
     *
     * <p>The offset is left back at the top, because the sections after this one
     * read the window the run opened on.
     */
    private static void keys(Screen mods) throws ReflectiveOperationException {
        java.lang.reflect.Field offset = mods.getClass().getDeclaredField("scroll");
        offset.setAccessible(true);
        offset.setInt(mods, 0);
        rebuild(mods);

        System.out.println("keyDown=" + press(mods, 264) + " at=" + offset.getInt(mods));
        System.out.println("keyPageDown=" + press(mods, 267) + " at=" + offset.getInt(mods));
        System.out.println("keyPageUp=" + press(mods, 266) + " at=" + offset.getInt(mods));
        System.out.println("keyUp=" + press(mods, 265) + " at=" + offset.getInt(mods));
        System.out.println("keyOther=" + press(mods, 65) + " at=" + offset.getInt(mods));
        // Escape is the game's: the stand-in closes the screen on it and says
        // so, which is the only way a key this screen leaves alone can be told
        // from a key it swallows.
        System.out.println("keyEscape=" + press(mods, 256) + " at=" + offset.getInt(mods));

        // Done is drawn by every layout, so the focus it holds has to survive a
        // scroll whatever the window shows.
        Button done = button(mods, "gui.done");
        mods.setFocused(done);
        boolean scrolled = press(mods, 264);
        System.out.println("focusDone=" + scrolled
                + " kept=" + (mods.getFocused() == button(mods, "gui.done")));
        if (scrolled) {
            offset.setInt(mods, 0);
            rebuild(mods);
        }

        // The settings entries are the last rows of the list, so the window is
        // moved to the end to draw one. The offset is bounded on the layout, so
        // naming a row past the end is how the end is named without counting.
        offset.setInt(mods, Integer.MAX_VALUE);
        rebuild(mods);
        Button entry = settingsEntry(mods, null);
        if (entry == null) {
            System.out.println("focusHeld=none");
        } else {
            String label = entry.message().text();
            mods.setFocused(entry);
            boolean moved = press(mods, 265);
            Button again = settingsEntry(mods, label);
            System.out.println("focusMoved=" + moved + " drawn=" + (again != null)
                    + " kept=" + (again != null && mods.getFocused() == again));
        }

        offset.setInt(mods, 0);
        rebuild(mods);
    }

    /**
     * Presses a child screen's Done and names what is on screen afterwards.
     *
     * <p>Both child screens are opened from the mods screen, so closing has to put
     * a mods screen back rather than the game's own nothing, which is the title.
     * The crossing counted is {@code setScreen}, a hooked seam; the settings
     * save reaches native code too but only seam hooks pass through the guard
     * that counts, so the settings run reports it separately.
     */
    private static String closes(Screen child) throws ReflectiveOperationException {
        long before = invocations();
        button(child, "gui.done").press();
        Screen after = Minecraft.getInstance().screen();
        return (after == null ? "null" : after.getClass().getName())
                + " crossed=" + (invocations() - before);
    }

    /** Presses one key and says whether the screen used it. */
    private static boolean press(Screen mods, int key) {
        return mods.keyPressed(new KeyEvent(key, 0, 0));
    }

    /** The settings entry drawn under one label, or the first one drawn for a null label. */
    private static Button button(Screen mods, String label) {
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof Button drawn && drawn.message().text().equals(label)) {
                return drawn;
            }
        }
        throw new IllegalStateException("no button reads " + label);
    }

    private static Button settingsEntry(Screen mods, String label) {
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof Button drawn
                    && drawn.message().text().startsWith("Settings: ")
                    && (label == null || drawn.message().text().equals(label))) {
                return drawn;
            }
        }
        return null;
    }

    /** Sets one of the screen's own flags, which a fed payload never sets itself. */
    private static void set(Screen screen, String name, boolean value)
            throws ReflectiveOperationException {
        java.lang.reflect.Field field = screen.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.setBoolean(screen, value);
    }

    /**
     * Scrolls the window to the end of the list and back, checking every stop.
     *
     * <p>What this proves is the whole of the scrolling contract, at whatever
     * window size the run was given: the drawn rows are always the slice of the
     * list the offset names, every row is reached exactly once and in order,
     * Profiling and Done are drawn at every stop, nothing is drawn below the
     * window, every drawn row renders whole, and the summary says where in the
     * list a player is. A notch past either end must move nothing, which is what
     * the offset's one clamp is there for.
     *
     * <p>And the surface is read once however far the window travels, which is
     * two assertions rather than one. {@code stable} says the list is the list it
     * was before the walk: a screen that read the surface again on every layout
     * would append the set to itself and the rows would grow, which is what the
     * read guard stops. {@code perLayout} says a layout costs exactly one hook
     * invocation, which is the game's own {@code Screen.init} and not the surface
     * read at all — {@code Hook.nativeInvocations} counts rewritten call sites,
     * and the screen surface is an export the transformer never rewrote. So the
     * count cannot see the read, and it is here to catch a new per-layout hook
     * rather than to stand in for {@code stable}.
     *
     * <p>The summary's own fit is reported per stop rather than asserted with the
     * rows. A row is split to the width it is drawn at and so fits by
     * construction; the summary is one line that is not, and whether it outgrows
     * a row is a fact about the window rather than a defect in the walk.
     */
    private static void walk(Screen mods) throws ReflectiveOperationException {
        List<?> all = (List<?>) mods.getClass().getMethod("rowLabels").invoke(mods);
        java.lang.reflect.Field offset = mods.getClass().getDeclaredField("scroll");
        offset.setAccessible(true);
        long hooks = invocations();

        // A notch with no vertical component. It must move nothing and lay
        // nothing out, which is what the layout count says: a rebuild costs one
        // hook invocation and this costs none.
        int resting = offset.getInt(mods);
        long quiet = invocations();
        boolean sideways = mods.mouseScrolled(0.0, 0.0, 1.0, 0.0);
        System.out.println("sideways=" + sideways
                + " held=" + (offset.getInt(mods) == resting)
                + " layouts=" + (invocations() - quiet));

        if (!mods.mouseScrolled(0.0, 0.0, 0.0, -1.0)) {
            // Either the whole list is on screen or the window has room for no
            // row at all. Both refuse the wheel, and neither may move the offset:
            // a window too short to draw a row that still counted notches would
            // open part of the way down the list the next time it was tall
            // enough to draw one.
            System.out.println("scrollable=false rows=" + all.size()
                    + " shown=" + drawn(mods).size()
                    + " held=" + (offset.getInt(mods) == resting)
                    + " hooks=" + (invocations() - hooks));
            return;
        }
        // The wheel above moved the window, so the walk starts from the top again.
        offset.setInt(mods, 0);
        rebuild(mods);

        // One for the probe above and one for the rebuild, both of which laid the
        // screen out again.
        int layouts = 2;
        List<String> reached = new ArrayList<>(drawn(mods));
        int shown = reached.size();
        if (shown == 0) {
            // A window with room for no row accepted a notch, which is the one
            // thing it must not do. Reported rather than walked, because a walk
            // over an empty window reads a row that is not there and the stack
            // trace it throws says less than this line does.
            System.out.println("walkEmpty=true rows=" + all.size()
                    + " at=" + offset.getInt(mods));
            return;
        }
        int positions = 1;
        boolean slices = reached.equals(all.subList(0, shown));
        boolean trailing = trailing(mods);
        boolean within = within(mods);
        boolean fits = fits(mods);
        boolean aligned = aligned(mods);
        System.out.println("walkSummary=" + summary(mods));

        while (offset.getInt(mods) + shown < all.size()) {
            int from = offset.getInt(mods);
            mods.mouseScrolled(0.0, 0.0, 0.0, -1.0);
            layouts++;
            positions++;
            List<String> window = drawn(mods);
            int at = offset.getInt(mods);
            if (at == from) {
                // The end of the list is reached by moving, so a notch that
                // moves nothing before the end is reached never reaches it.
                // Reported rather than retried: this loop would otherwise run
                // for as long as the harness is allowed to.
                System.out.println("walkStuck=true at=" + at
                        + " rows=" + all.size() + " shown=" + shown);
                return;
            }
            slices &= window.equals(all.subList(at, at + shown));
            trailing &= trailing(mods);
            within &= within(mods);
            fits &= fits(mods);
            aligned &= aligned(mods);
            reached.add(window.get(window.size() - 1));
            System.out.println("walkSummary=" + summary(mods));
        }

        // A notch past each end, which moves nothing, lays nothing out, and is
        // refused so the game keeps it; then the way back up.
        int end = offset.getInt(mods);
        boolean held = !mods.mouseScrolled(0.0, 0.0, 0.0, -1.0)
                && offset.getInt(mods) == end && drawn(mods).equals(all.subList(end, all.size()));
        for (int step = end; step > 0; step--) {
            mods.mouseScrolled(0.0, 0.0, 0.0, 1.0);
            layouts++;
            int at = offset.getInt(mods);
            held &= at == step - 1 && drawn(mods).equals(all.subList(at, at + shown));
        }
        held &= !mods.mouseScrolled(0.0, 0.0, 0.0, 1.0)
                && offset.getInt(mods) == 0 && drawn(mods).equals(all.subList(0, shown));

        List<?> after = (List<?>) mods.getClass().getMethod("rowLabels").invoke(mods);
        System.out.println("walkRows=" + all.size() + " shown=" + shown + " positions=" + positions);
        System.out.println("walkOrder=" + reached.equals(all) + " slices=" + slices + " held=" + held
                + " stable=" + after.equals(all));
        System.out.println("walkTrailing=" + trailing + " within=" + within + " fits=" + fits
                + " aligned=" + aligned);
        System.out.println("walkLayouts=" + layouts
                + " perLayout=" + ((invocations() - hooks) == layouts));
    }

    /** The list rows on screen, which is every drawn widget but the summary and the two buttons. */
    private static List<String> drawn(Screen mods) {
        List<String> window = new ArrayList<>();
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof StringWidget line) {
                window.add(line.text());
            } else if (widget instanceof Button entry) {
                window.add(entry.message().text());
            }
        }
        // The summary is the first, and Profiling and Done are the last two.
        return window.subList(1, window.size() - 2);
    }

    /** The summary line and whether it renders whole, which is the first widget drawn. */
    private static String summary(Screen mods) {
        StringWidget line = (StringWidget) mods.widgets().get(0);
        return line.fits() + " " + line.text();
    }

    /** Whether Profiling and Done are the last two widgets drawn. */
    private static boolean trailing(Screen mods) {
        List<GuiEventListener> widgets = mods.widgets();
        int count = widgets.size();
        return count >= 3
                && widgets.get(count - 2) instanceof Button profiling
                && profiling.message().text().equals("Profiling")
                && widgets.get(count - 1) instanceof Button done
                && done.message().text().equals("gui.done");
    }

    /** Whether every drawn widget sits inside the window. */
    private static boolean within(Screen mods) {
        int bottom = 0;
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof StringWidget line) {
                bottom = Math.max(bottom, line.y() + line.height());
            } else if (widget instanceof Button drawn) {
                bottom = Math.max(bottom, drawn.y() + drawn.height());
            }
        }
        return bottom <= mods.height;
    }

    /** Whether every drawn row below the summary renders whole rather than clipped. */
    private static boolean fits(Screen mods) {
        List<GuiEventListener> widgets = mods.widgets();
        for (int index = 1; index < widgets.size(); index++) {
            if (widgets.get(index) instanceof StringWidget line && !line.fits()) {
                return false;
            }
        }
        return true;
    }

    /** Whether every drawn line starts at the left edge of the box drawn in. */
    private static boolean aligned(Screen mods) {
        for (GuiEventListener widget : mods.widgets()) {
            if (widget instanceof StringWidget line && line.textX() != line.x()) {
                return false;
            }
        }
        return true;
    }

    /** Lays the screen out again without going through the wheel. */
    private static void rebuild(Screen mods) throws ReflectiveOperationException {
        Method again = Screen.class.getDeclaredMethod("rebuildWidgets");
        again.setAccessible(true);
        again.invoke(mods);
    }

    /**
     * How many hook calls have reached the native library.
     *
     * <p>Reflectively, because this class is compiled without the agent jar on
     * its class path, exactly as a real launch is.
     */
    private static long invocations() throws ReflectiveOperationException {
        Object count = Class.forName("dev.ravel.jvm.Hook")
                .getMethod("nativeInvocations")
                .invoke(null);
        return ((Long) count).longValue();
    }

    /** One field on the screen's wire, which states the length it is read with. */
    private static String field(String value) {
        return value.length() + ":" + value;
    }
}
