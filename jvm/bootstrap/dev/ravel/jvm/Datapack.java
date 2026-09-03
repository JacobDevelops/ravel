package dev.ravel.jvm;

import java.lang.reflect.Method;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.function.Consumer;

/**
 * Puts the mods' data pack where the game will read it, on both paths into a world.
 *
 * <p><b>Selected and loaded are different things.</b> The game picks packs by
 * scanning a directory and reads their contents in a separate step, and a path
 * into a world may do the first without the second. Ravel has to make both, and
 * the two halves of this class are the two places that takes.
 *
 * <p>{@link #write(Object)} runs where a world's pack repository is built. That
 * is what gets the pack selected, and for a world being opened it is enough:
 * opening reloads the world's data after it. The crossing that admits a world is
 * later still, handed a stem the game built from a repository it had already
 * configured, so a pack written from there is read on the world's next open.
 *
 * <p>{@link #stage(Object)} runs on the create-world screen, because creating a
 * world reloads nothing. The screen loads recipes, tags and loot tables once when
 * it opens, and the created world is handed those same resources. So the pack is
 * put where the screen's own load will see it — the staging directory the game
 * copies into the new world — and the screen is asked to load again. That is the
 * path the game already runs when a player adds a data pack by hand, and it costs
 * what that path costs: one data load, with the game's own loading screen in
 * front of it. It is asked for only when something was staged, so a launch whose
 * mods supply no data pays neither.
 *
 * <p><b>Nothing here decides whether a world opens.</b> The pack is a
 * convenience the game would have started without, so a failure is reported and
 * this returns. That is the opposite of {@link Persistence}, deliberately:
 * losing a mod's recipes for a session is not losing a player's saved work.
 */
final class Datapack {

    /**
     * The screens already staged, held weakly so a closed one is forgotten.
     *
     * <p>The seam is the screen building its widgets, which the game runs again
     * every time the screen is shown or resized — and the reload staging asks for
     * ends by showing this screen again, so an unguarded adapter would stage,
     * reload, stage and never stop. Identity is the right key because two
     * create-world screens are two screens however alike their settings are, and
     * {@code Screen} defines no equality of its own.
     */
    private static final Set<Object> STAGED =
            Collections.synchronizedSet(Collections.newSetFromMap(new WeakHashMap<>()));

    /** Nothing was staged, so there is nothing to select and nothing to load. */
    private static final int UNSTAGED = 0;

    /** The staged pack's files are not the ones that were there. */
    private static final int STAGED_ANEW = 2;

    private Datapack() {
    }

    /**
     * Writes the pack for the world this access is holding.
     *
     * @param access the selected {@code LevelStorageAccess}, held opaquely
     */
    static void write(Object access) {
        try {
            Hook.ravelWriteWorldDatapack(Persistence.worldPath(access));
        } catch (Throwable unwritten) {
            // Deliberately everything. This runs inside a call the game makes on
            // its way into a world, and an escape here would cost the player the
            // world over a file the world does not need to open.
            Agent.diagnose("the mods' data pack could not be written into this world ("
                    + unwritten.getClass().getName() + ")");
        }
    }

    /**
     * Stages the pack on one create-world screen and asks it to load again.
     *
     * @param screen the {@code CreateWorldScreen} being built, held opaquely
     */
    static void stage(Object screen) {
        if (!STAGED.add(screen)) {
            return;
        }
        try {
            apply(screen);
        } catch (Throwable unstaged) {
            // Deliberately everything, for the reason above and one more: this is
            // a screen the player is looking at, and a screen that throws while
            // building its widgets is a client that cannot create a world at all.
            Agent.diagnose("the mods' data pack could not be staged for a world being created ("
                    + unstaged.getClass().getName() + ")");
        }
    }

    /** The steps, separated so the guard above reads as one thing. */
    private static void apply(Object screen) throws ReflectiveOperationException {
        Class<?> type = screen.getClass();
        Object uiState =
                Members.method(type, GameMembers.CREATE_WORLD_SCREEN_GET_UI_STATE).invoke(screen);
        Object settings = Members.method(
                uiState.getClass(), GameMembers.WORLD_CREATION_UI_STATE_GET_SETTINGS)
                .invoke(uiState);
        Object configuration = Members.method(
                settings.getClass(), GameMembers.WORLD_CREATION_CONTEXT_DATA_CONFIGURATION)
                .invoke(settings);

        // Asking for the screen's own configuration rather than the game's
        // default is what keeps a world being recreated from re-enabling a pack
        // its player turned off: the repository comes back selecting exactly
        // what this call is given.
        Object selection = Members.method(
                type, GameMembers.CREATE_WORLD_SCREEN_GET_DATA_PACK_SELECTION_SETTINGS)
                .invoke(screen, configuration);
        if (selection == null) {
            // The game could not make its staging directory and has already told
            // the player so. There is nowhere to put the pack and nothing to add.
            return;
        }

        int staged = Hook.ravelWriteStagedDatapack(String.valueOf(pairPart(selection, "getFirst")));
        if (staged == UNSTAGED) {
            // Nothing was staged, so there is nothing to reload for. A launch
            // whose mods supply no data reaches here, and it should cost the
            // player exactly what it did before Ravel was installed.
            return;
        }

        Object repository = pairPart(selection, "getSecond");
        switch (select(repository, Hook.ravelPackId())) {
            case REFUSED -> Agent.diagnose(
                    "the game did not find the mods' data pack where it was staged, so this world "
                            + "will have none of their recipes or tags");
            // False asks for no experimental-content dialog. The pack is Ravel's
            // own and the player did not choose it, so there is nothing to
            // confirm. The selection this hands over is not the one the screen
            // holds, which is what makes the call load rather than return.
            case ADDED -> Members.method(
                    type, GameMembers.CREATE_WORLD_SCREEN_TRY_APPLY_NEW_DATA_PACKS)
                    .invoke(screen, repository, false, reportValidationFailure());
            case ALREADY -> {
                if (staged == STAGED_ANEW) {
                    // The pack was already selected and its files changed under
                    // that selection, so the call above would compare the two
                    // configurations equal and load nothing — and the world
                    // would be created from the data loaded before the
                    // replacement. This is the load that call runs when it does
                    // decide to, reached with the configuration the screen
                    // already holds because the selection is not what changed.
                    Members.method(type, GameMembers.CREATE_WORLD_SCREEN_APPLY_NEW_PACK_CONFIG)
                            .invoke(screen, repository, configuration, reportValidationFailure());
                }
            }
        }
    }

    /** What putting the staged pack into a repository's selection did. */
    private enum Selection {
        /** The repository does not have the pack, so selecting it did nothing. */
        REFUSED,
        /** The pack was not selected and now is. */
        ADDED,
        /** The pack was selected already, so the selection is unchanged. */
        ALREADY
    }

    /**
     * Puts the staged pack into a repository's selection, keeping what is there.
     *
     * <p>Both steps are Ravel's to take. The repository was scanned when the
     * screen handed it over, which was before the pack existed, so a rescan is
     * what makes the pack something this repository has; and the call this
     * precedes reads a selection rather than making one, so nothing else would
     * ever put the pack in it. Adding to the ids already selected rather than
     * replacing them is what keeps a world being recreated from re-enabling a
     * pack its player turned off.
     *
     * <p>The answer is read back rather than assumed, because setting a selection
     * keeps only the ids the last scan found: a pack the game could not read is
     * one this leaves entirely unselected, and the caller has to say so rather
     * than hand the player a world that quietly has no mod data.
     */
    private static Selection select(Object repository, String id)
            throws ReflectiveOperationException {
        Class<?> type = repository.getClass();
        Members.method(type, GameMembers.PACK_REPOSITORY_RELOAD).invoke(repository);
        Collection<?> already = selected(repository);
        if (already.contains(id)) {
            return Selection.ALREADY;
        }
        Set<Object> selection = new LinkedHashSet<>(already);
        // Last, which is the highest priority the game gives a pack: where a
        // player's pack and a mod's supply the same file, the mod's wins.
        selection.add(id);
        Members.method(type, GameMembers.PACK_REPOSITORY_SET_SELECTED)
                .invoke(repository, selection);
        return selected(repository).contains(id) ? Selection.ADDED : Selection.REFUSED;
    }

    /** What the repository selects now. */
    private static Collection<?> selected(Object repository) throws ReflectiveOperationException {
        return (Collection<?>) Members.method(
                repository.getClass(), GameMembers.PACK_REPOSITORY_GET_SELECTED_IDS)
                .invoke(repository);
    }

    /**
     * One half of the pair the selection call hands back.
     *
     * <p>Reached by plain reflection rather than through {@link GameMembers}
     * because the pair is a library type rather than a game one: it keeps its
     * name under every version, and the reviewed surface is for the names that do
     * not. The agent jar compiles against nothing, so it cannot name the type
     * either way.
     */
    private static Object pairPart(Object pair, String half) throws ReflectiveOperationException {
        Method part = pair.getClass().getMethod(half);
        part.setAccessible(true);
        return part.invoke(pair);
    }

    /**
     * What the game calls if it refuses the staged pack.
     *
     * <p>The game uses this only on the path where validation failed and the
     * player chose to go back, which is the failure branch of a failure branch.
     * Ravel has nothing to put back there — it did not open the pack screen — so
     * this says what happened instead of doing nothing quietly.
     */
    private static Consumer<Object> reportValidationFailure() {
        return refused -> Agent.diagnose(
                "the game refused the mods' data pack while creating a world, so this world will "
                        + "have none of their recipes or tags");
    }
}
