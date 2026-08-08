import dev.ravel.jvm.screen.RavelProfilingScreen;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;

/**
 * Builds the profiling screen at the window sizes a player's GUI scale produces
 * and checks that every line it draws can be read.
 *
 * <p>The defect this stands against was not a crash: the screen collected a real
 * report and drew it, and the two longest lines rendered only their tails,
 * because a value wider than its box scrolls to the cursor the box parks at its
 * end. So the check is not that the screen built — it always did — but that
 * every value fits the width it is drawn at, that nothing is drawn past the
 * bottom, and that the accuracy statement reads back whole from the rows it was
 * split across.
 */
public final class ProfilingScreenMain {

    /**
     * The statement the report exists to make, as `ravel-observability` writes
     * it. Written out here rather than read from the payload: a check that
     * compared the screen against the same string the screen read would pass on
     * a screen that drew nothing at all.
     */
    private static final String CAVEAT =
            "A mod is charged for host work it calls back into. The window is entry to return, so "
                    + "a mod that asks Ravel to resolve a registry key is charged for the lookup as "
                    + "well as for its own code.";

    /** The heading the statement is drawn under. */
    private static final String HEADING = "What these numbers cannot tell apart:";

    /**
     * Window sizes in the units the screen places widgets in, which is what a
     * GUI scale divides the monitor down to. 320x240 is the smallest the game
     * guarantees, and 480x270 is what auto scale gives a 1080p monitor.
     */
    private static final int[][] SIZES = {{320, 240}, {480, 270}, {640, 360}, {960, 540}};

    public static void main(String[] args) throws ReflectiveOperationException {
        Method init = RavelProfilingScreen.class.getDeclaredMethod("init");
        init.setAccessible(true);

        // One screen per size. The stand-in screen keeps every widget it is ever
        // given, so a second `init` on the same one would be read as a longer
        // report rather than as a second layout of the same one.
        for (int[] size : SIZES) {
            Screen screen = RavelProfilingScreen.open();
            screen.width = size[0];
            screen.height = size[1];
            init.invoke(screen);

            List<String> lines = new ArrayList<>();
            boolean fits = true;
            int bottom = 0;
            for (GuiEventListener widget : screen.widgets()) {
                if (widget instanceof EditBox box) {
                    lines.add(box.getValue());
                    fits &= box.fits();
                    bottom = Math.max(bottom, box.y() + box.height());
                } else if (widget instanceof Button button) {
                    bottom = Math.max(bottom, button.y() + button.height());
                }
            }

            System.out.println("size=" + size[0] + "x" + size[1]
                    + " rows=" + lines.size()
                    + " fits=" + fits
                    + " within=" + (bottom <= screen.height)
                    + " caveat=" + (caveat(lines).equals(CAVEAT) ? "whole" : "truncated"));
        }
    }

    /** The statement read back out of the rows it was split across. */
    private static String caveat(List<String> lines) {
        int heading = lines.indexOf(HEADING);
        if (heading < 0) {
            return "";
        }
        return String.join(" ", lines.subList(heading + 1, lines.size()));
    }
}
