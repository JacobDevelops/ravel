package dev.ravel.jvm.screen;

import dev.ravel.jvm.Hook;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.components.Button;
// The binary name is the type's name here, for the reason RavelSettingsScreen
// records: the pinned version reaches every interactive widget through a nested
// type and the stubs carry the binary name the JVM links on.
import net.minecraft.client.gui.components.Button$OnPress;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FormattedText;

/**
 * Ravel's opt-in profiling report.
 *
 * <p>{@code ROADMAP.md}'s M3-W6 row asks for collectors and a profiling UI or
 * report, and for two things about it to be explicit: how accurate the per-mod
 * attribution is, and which metrics are unsupported. This screen is where a
 * player reads all three, and it is ADG-6's mechanism a third time — one batched
 * read on the same length-prefixed framing, the same widget vocabulary, and no
 * drawing outside this class.
 *
 * <p><b>Off is a first-class answer.</b> Profiling is off unless a launch set
 * {@code -Dravel.profiling=true}, and a screen opened on a normal launch says so
 * and names the switch. It does not draw a table of zeroes: a mod that was never
 * measured and a mod that cost nothing are different facts, and a column of
 * zeroes states the second when only the first is true.
 *
 * <p><b>An unsupported metric is drawn, not omitted.</b> The payload carries
 * every metric with a word for how well it can be measured — {@code exact},
 * {@code approximate}, {@code unsupported} — and the sentence behind that word.
 * Leaving the unsupported ones out would make them indistinguishable from
 * metrics that measured zero, which is the silence the row exists to end.
 *
 * <p><b>The accuracy statement is part of the report.</b> A per-mod nanosecond
 * total charges a mod for host work it called into and for time the scheduler
 * gave to something else. That is not a footnote to the number, it is what the
 * number means, so it is drawn under the same heading rather than left to
 * documentation a player will not read.
 *
 * <p><b>Every line is split to the width it is drawn at.</b> A row wider than
 * the widget carrying it renders only its tail: {@link EditBox#setValue} parks
 * the cursor at the end of the value and the box scrolls its display window to
 * the cursor, so the beginning of a long line is scrolled out of sight. The
 * accuracy statement below is the longest line here and it is the one that must
 * be read, which makes splitting a correctness fix rather than a cosmetic one.
 * The game's font does the splitting — {@code splitIgnoringLanguage} measures
 * glyphs and breaks on words — because a screen that counted characters instead
 * would be right only for scripts whose glyphs happen to be one width.
 *
 * <p><b>Known limit: no scrolling.</b> The same limit
 * {@link RavelModsScreen} records, for the same reason and with the same
 * remedy. Mods arrive costliest first, so a set larger than {@link #MAX_ROWS}
 * loses the rows a player was least likely to be looking for. Splitting spends
 * rows and a player's GUI scale decides how many the window has, so that trade
 * is now made against a measured budget rather than a fixed count: {@link #rows}
 * measures the window, {@link #init} gives it out in a stated order, and a row
 * drawn past the bottom is exactly as unreadable as one drawn off the side. On
 * the smallest windows the game produces that leaves the summary and the
 * accuracy statement alone. Seeding a scroll widget is what stops the trade
 * having to be made at all.
 *
 * <p>Every widget here is one the game builds without this class drawing
 * anything, for the reason {@link RavelSettingsScreen} records at length.
 */
public final class RavelProfilingScreen extends Screen {

    /** Where the first row sits, below the title. */
    private static final int TOP = 32;

    /** The distance between one row and the next. */
    private static final int ROW = 22;

    private static final int WIDGET_WIDTH = 320;
    private static final int WIDGET_HEIGHT = 20;

    /** What a bordered {@link EditBox}'s own border takes from the text inside it. */
    private static final int TEXT_INSET = 8;

    /** The most mod rows this screen draws, before the window's height cuts it further. */
    private static final int MAX_ROWS = 8;

    /** What the accuracy statement is drawn under. */
    private static final String HEADING = "What these numbers cannot tell apart:";

    /** The mods that were entered, costliest first. */
    private final List<Entry> entries = new ArrayList<>();

    /** What a number here can and cannot tell apart. */
    private List<String> accuracy = List.of();

    /** Every metric and what can be said about it. */
    private final List<Metric> metrics = new ArrayList<>();

    /** Whether the collectors were running. */
    private boolean on;

    /** The switch that turns them on, as the loader spells it. */
    private String profilingSwitch = "";

    /** Ticks the collectors saw. */
    private int ticks;

    /** The threshold above which a callback was counted as slow. */
    private int slowMicros;

    /** Whether the payload read at all. */
    private boolean read;

    private RavelProfilingScreen() {
        super(Component.translatable("ravel.profiling.title"));
    }

    /**
     * Builds the screen.
     *
     * <p>Called reflectively by {@code dev.ravel.jvm.Screens}, which cannot name
     * this type: {@code Screens} lives in the agent jar and ADG-1 forbids that
     * jar naming a game type, which this class's supertype is.
     *
     * @return a screen the caller hands to the game
     */
    public static Screen open() {
        return new RavelProfilingScreen();
    }

    /** The mods this report covers, costliest first. */
    public List<Entry> entries() {
        return List.copyOf(entries);
    }

    /** Every metric the report describes. */
    public List<Metric> metrics() {
        return List.copyOf(metrics);
    }

    /** Whether the collectors were running when this report was taken. */
    public boolean isOn() {
        return on;
    }

    /**
     * Reads the report once and builds the rows.
     *
     * <p>A payload that does not read leaves everything empty and says so rather
     * than drawing part of one. Half a profile is worse than none: it looks like
     * a mod cost less than it did.
     *
     * <p>Splitting spends rows and the window has a fixed number of them, so the
     * order things are taken out in is a decision rather than an accident. The
     * summary and the accuracy statement are claimed first and are never cut:
     * one says what is being looked at and the other says what it means, and a
     * number drawn without them is the misreading this screen exists to stop.
     * What is left over goes, in order, to the heading over that statement, to
     * the metrics no collector can measure, and to the mod table. The last two
     * are shortened rather than dropped, because a metric silently left out
     * reads as one that measured zero and a mod silently left out reads as one
     * that cost nothing, which is the same silence in both cases.
     */
    @Override
    protected void init() {
        super.init();
        entries.clear();
        metrics.clear();
        read = parse(Hook.ravelProfilingReport());

        List<String> above = wrap(summary());
        List<String> caveat = accuracy.isEmpty() ? List.of() : wrap(List.of(accuracy.get(0)));
        int budget = rows() - above.size() - caveat.size();

        // The heading names the statement under it and the statement says the
        // thing. A window with room for one of the two keeps the statement.
        List<String> heading = caveat.isEmpty() || budget < 1 ? List.of() : List.of(HEADING);
        budget -= heading.size();

        List<String> unmeasurable = unmeasurable(budget);
        budget -= unmeasurable.size();

        int y = TOP;
        for (String line : above) {
            addRenderableWidget(label(line, y));
            y += ROW;
        }
        for (String line : table(budget)) {
            addRenderableWidget(label(line, y));
            y += ROW;
        }
        for (String line : unmeasurable) {
            addRenderableWidget(label(line, y));
            y += ROW;
        }
        for (String line : heading) {
            addRenderableWidget(label(line, y));
            y += ROW;
        }
        for (String line : caveat) {
            addRenderableWidget(label(line, y));
            y += ROW;
        }

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

    /**
     * The mod rows that fit in what the rest of the report left, costliest first.
     *
     * @param budget how many rows are free between the summary and the statement
     *     below it
     * @return the rows to draw, ending in a line naming what was left out
     */
    private List<String> table(int budget) {
        if (budget < 1) {
            return List.of();
        }
        List<String> drawn = new ArrayList<>();
        int shown = 0;
        for (Entry entry : entries) {
            if (shown == MAX_ROWS) {
                break;
            }
            List<String> row = wrap(List.of(entry.line()));
            // The last entry needs no line saying what was left out. Every other
            // one might, so the row that line would take is held back until it
            // turns out not to be needed.
            int held = shown + 1 < entries.size() ? 1 : 0;
            if (drawn.size() + row.size() + held > budget) {
                break;
            }
            drawn.addAll(row);
            shown++;
        }
        if (shown < entries.size()) {
            drawn.add((entries.size() - shown) + " cheaper mods not shown");
        }
        return drawn;
    }

    /**
     * How many rows this window has between the title and the done button.
     *
     * <p>Measured rather than fixed, because a player's GUI scale is what decides
     * how tall this window is in the units everything here is placed in.
     *
     * @return at least one row, however small the window claims to be
     */
    private int rows() {
        return Math.max(1, (height - TOP - ROW - WIDGET_HEIGHT) / ROW);
    }

    /**
     * Every line the given strings draw, each split to the width it is drawn at.
     *
     * <p>The split runs against the text a box has room for rather than against
     * the box, because {@link EditBox} spends {@link #TEXT_INSET} of its width on
     * its own border and a line that fits the border does not fit the text.
     *
     * @param texts the strings to draw, one or more rows each
     * @return the rows, in order
     */
    private List<String> wrap(List<String> texts) {
        List<String> lines = new ArrayList<>();
        for (String text : texts) {
            for (Object piece :
                    font.splitIgnoringLanguage(FormattedText.of(text), WIDGET_WIDTH - TEXT_INSET)) {
                lines.add(((FormattedText) piece).getString());
            }
        }
        return lines;
    }

    /**
     * The lines above the table, saying what the player is looking at.
     *
     * <p>An off report says so and names the switch. That is the whole report on
     * a normal launch, and it is the honest one: nothing was collected, so there
     * is no number to show and no number that would be a zero.
     */
    private List<String> summary() {
        if (!read) {
            return List.of("The profiling report could not be read.");
        }
        if (!on) {
            return List.of(
                    "Profiling is off, so nothing was collected.",
                    "Launch with " + profilingSwitch + " to turn it on.");
        }
        if (entries.isEmpty()) {
            return List.of("Profiling is on and no mod has been entered yet.");
        }
        return List.of(
                entries.size() + (entries.size() == 1 ? " mod over " : " mods over ")
                        + ticks + (ticks == 1 ? " tick" : " ticks"),
                "Slow above " + slowMicros + " us. Times are wall-clock, not CPU time.");
    }

    /**
     * One line naming every metric no collector here can measure.
     *
     * <p>Named rather than left out, and named even when profiling is off,
     * because what Ravel cannot measure is true of the build rather than of the
     * run.
     */
    private List<String> notMeasured() {
        List<String> names = unmeasured();
        if (names.isEmpty()) {
            return List.of();
        }
        return List.of("Not measurable in this build: " + String.join(", ", names));
    }

    /** Every metric no collector here can measure, by name. */
    private List<String> unmeasured() {
        List<String> names = new ArrayList<>();
        for (Metric metric : metrics) {
            if (metric.support.equals("unsupported")) {
                names.add(metric.name);
            }
        }
        return names;
    }

    /**
     * What this window has room to say about the metrics it cannot measure.
     *
     * <p>Naming them is the point: a metric left out entirely reads as one that
     * measured zero, which is the silence {@link #notMeasured} exists to end. A
     * window with no room for the names still has room to say the names are
     * there, and that keeps the two apart. Only a window with no room at all
     * says nothing.
     *
     * @param budget how many rows are free below the summary
     * @return the rows to draw, which may be none
     */
    private List<String> unmeasurable(int budget) {
        List<String> named = wrap(notMeasured());
        if (named.size() <= budget) {
            return named;
        }
        int count = unmeasured().size();
        if (count == 0) {
            return List.of();
        }
        List<String> shortened = wrap(List.of(count + (count == 1 ? " metric" : " metrics")
                + " not measurable in this build, not shown"));
        return shortened.size() <= budget ? shortened : List.of();
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

    /**
     * Reads the payload into the report.
     *
     * @return whether the whole payload read
     */
    private boolean parse(String payload) {
        if (payload == null) {
            return false;
        }
        Wire wire = new Wire(payload);
        String version = wire.field();
        if (version == null || !version.equals(Wire.VERSION)) {
            return false;
        }
        String running = wire.field();
        String switchName = wire.field();
        int reportedTicks = wire.count();
        int reportedSlow = wire.count();
        int mods = wire.count();
        if (running == null
                || switchName == null
                || reportedTicks < 0
                || reportedSlow < 0
                || mods < 0) {
            return false;
        }

        List<Entry> pendingEntries = new ArrayList<>();
        for (int index = 0; index < mods; index++) {
            Entry entry = Entry.read(wire);
            if (entry == null) {
                return false;
            }
            pendingEntries.add(entry);
        }

        int sentences = wire.count();
        if (sentences < 0) {
            return false;
        }
        List<String> pendingAccuracy = new ArrayList<>();
        for (int index = 0; index < sentences; index++) {
            String sentence = wire.field();
            if (sentence == null) {
                return false;
            }
            pendingAccuracy.add(sentence);
        }

        int described = wire.count();
        if (described < 0) {
            return false;
        }
        List<Metric> pendingMetrics = new ArrayList<>();
        for (int index = 0; index < described; index++) {
            Metric metric = Metric.read(wire);
            if (metric == null) {
                return false;
            }
            pendingMetrics.add(metric);
        }
        if (!wire.finished()) {
            return false;
        }

        on = running.equals("true");
        profilingSwitch = switchName;
        ticks = reportedTicks;
        slowMicros = reportedSlow;
        entries.addAll(pendingEntries);
        accuracy = pendingAccuracy;
        metrics.addAll(pendingMetrics);
        return true;
    }

    /** One mod, as the collectors measured it. */
    public static final class Entry {

        String modId = "";
        String inclusiveNanos = "";
        String hookCalls = "";
        String eventCalls = "";
        String commandCalls = "";
        String slowCallbacks = "";
        String registryClaims = "";

        /** Reads one entry, or {@code null} if the payload runs out. */
        static Entry read(Wire wire) {
            Entry entry = new Entry();
            entry.modId = wire.field();
            entry.inclusiveNanos = wire.field();
            entry.hookCalls = wire.field();
            entry.eventCalls = wire.field();
            entry.commandCalls = wire.field();
            entry.slowCallbacks = wire.field();
            entry.registryClaims = wire.field();
            if (entry.modId == null
                    || entry.inclusiveNanos == null
                    || entry.hookCalls == null
                    || entry.eventCalls == null
                    || entry.commandCalls == null
                    || entry.slowCallbacks == null
                    || entry.registryClaims == null) {
                return null;
            }
            return entry;
        }

        /** The mod this entry describes. */
        public String modId() {
            return modId;
        }

        /**
         * The row this entry draws.
         *
         * <p>The counts stay text all the way from the loader. They are counters
         * over a session rather than quantities to compute with, and parsing them
         * here would only create a way to fail at it.
         */
        public String line() {
            StringBuilder row = new StringBuilder();
            row.append(modId)
                    .append(" — ")
                    .append(inclusiveNanos)
                    .append(" ns in ")
                    .append(hookCalls)
                    .append(" calls (")
                    .append(eventCalls)
                    .append(" ev, ")
                    .append(commandCalls)
                    .append(" cmd)");
            if (!slowCallbacks.equals("0")) {
                row.append(", ").append(slowCallbacks).append(" slow");
            }
            if (!registryClaims.equals("0")) {
                row.append(", ").append(registryClaims).append(" registered");
            }
            return row.toString();
        }
    }

    /** One metric, and how well this build can measure it. */
    public static final class Metric {

        String name = "";
        String unit = "";
        String support = "";
        String detail = "";

        /** Reads one metric, or {@code null} if the payload runs out. */
        static Metric read(Wire wire) {
            Metric metric = new Metric();
            metric.name = wire.field();
            metric.unit = wire.field();
            metric.support = wire.field();
            metric.detail = wire.field();
            if (metric.name == null
                    || metric.unit == null
                    || metric.support == null
                    || metric.detail == null) {
                return null;
            }
            return metric;
        }

        /** The metric's name. */
        public String name() {
            return name;
        }

        /** One of {@code exact}, {@code approximate}, or {@code unsupported}. */
        public String support() {
            return support;
        }

        /** Why it is approximate or unsupported, empty when it is exact. */
        public String detail() {
            return detail;
        }
    }
}
