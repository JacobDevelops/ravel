package dev.ravel.jvm.screen;

import net.minecraft.client.gui.components.Button;
// The binary name is the type's name here, for the reason RavelSettingsScreen
// records: the pinned version reaches every interactive widget through a nested
// type and the stubs carry the binary name the JVM links on.
import net.minecraft.client.gui.components.Button$OnPress;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/**
 * What a player sees when Ravel refuses to open a world.
 *
 * <p>ADG-7 gives Rust the decision and Java the cleanup and the presentation, so
 * this screen composes nothing: the diagnostic it draws is the text the open
 * crossing answered with, wrapped and nothing more. Inventing a friendlier
 * sentence here would state a reason Ravel did not decide.
 *
 * <p>It is drawn with ADG-6's mechanism, like every other Ravel screen — the
 * same widget vocabulary, the same uneditable text field standing in for a
 * label, and no drawing outside this class.
 *
 * <p>The recovery this screen names is the one ADG-7 defines: deleting Ravel's
 * saved-data file resets Ravel's state for that world and leaves the rest of it
 * untouched. It is named rather than performed, because deleting a player's data
 * is not a thing a refusal path should do on its own.
 */
public final class RavelWorldRefusedScreen extends Screen {

    private static final int TOP = 40;
    private static final int ROW = 22;
    private static final int WIDGET_WIDTH = 320;
    private static final int WIDGET_HEIGHT = 20;

    /** How many characters of the diagnostic one row carries. */
    private static final int PER_ROW = 60;

    /** How many rows the diagnostic may take before it is cut. */
    private static final int MAX_ROWS = 6;

    private final String diagnostic;

    private RavelWorldRefusedScreen(String diagnostic) {
        super(Component.translatable("ravel.world.refused.title"));
        this.diagnostic = diagnostic == null ? "" : diagnostic;
    }

    /**
     * Builds the screen.
     *
     * <p>Called reflectively by {@code dev.ravel.jvm.Screens}, which cannot name
     * this type: {@code Screens} lives in the agent jar and ADG-1 forbids that
     * jar naming a game type, which this class's supertype is.
     *
     * @param diagnostic the refusal text the open crossing answered with
     * @return a screen the caller hands to the game
     */
    public static Screen open(String diagnostic) {
        return new RavelWorldRefusedScreen(diagnostic);
    }

    /** The refusal text this screen was built to draw. */
    public String diagnostic() {
        return diagnostic;
    }

    @Override
    protected void init() {
        super.init();
        int y = TOP;
        addRenderableWidget(label("This world was not opened.", y));
        y += ROW;
        for (int start = 0, rows = 0;
                start < diagnostic.length() && rows < MAX_ROWS;
                start += PER_ROW, rows++) {
            addRenderableWidget(
                    label(diagnostic.substring(start, Math.min(diagnostic.length(), start + PER_ROW)), y));
            y += ROW;
        }
        addRenderableWidget(label("Deleting Ravel's saved data resets Ravel for this world.", y));
        y += ROW;

        addRenderableWidget(
                Button.builder(Component.translatable("gui.done"), new Button$OnPress() {
                            @Override
                            public void onPress(Button pressed) {
                                onClose();
                            }
                        })
                        .bounds(centre(), y + ROW, WIDGET_WIDTH, WIDGET_HEIGHT)
                        .build());
    }

    /** One uneditable text field, which is how this screen shows a line. */
    private EditBox label(String text, int y) {
        EditBox box =
                new EditBox(font, centre(), y, WIDGET_WIDTH, WIDGET_HEIGHT, Component.literal(text));
        box.setMaxLength(1024);
        box.setValue(text);
        box.setEditable(false);
        return box;
    }

    private int centre() {
        return (width - WIDGET_WIDTH) / 2;
    }
}
