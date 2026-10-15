package dev.ravel.jvm;

import java.lang.reflect.Method;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.function.Consumer;

/**
 * Writes packs before repository selection and stages them before create-world data loads.
 * Failures report missing recipes and tags without refusing world creation or opening.
 */
final class Datapack {

    // Widget rebuilds must not restage; weak keys avoid retaining closed screens.
    private static final Set<Object> STAGED =
            Collections.synchronizedSet(Collections.newSetFromMap(new WeakHashMap<>()));

    private static final int UNSTAGED = 0;

    private static final int STAGED_ANEW = 2;

    private Datapack() {
    }

    /** Writes the pack for the world held by this storage access. */
    static void write(Object access) {
        try {
            Hook.ravelWriteWorldDatapack(Persistence.worldPath(access));
        } catch (Throwable unwritten) {
            // Missing mod data must not prevent opening the world.
            Agent.diagnose("the mods' data pack could not be written into this world ("
                    + unwritten.getClass().getName() + ")");
        }
    }

    /** Stages the pack once per create-world screen and reloads its data when needed. */
    static void stage(Object screen) {
        if (!STAGED.add(screen)) {
            return;
        }
        try {
            apply(screen);
        } catch (Throwable unstaged) {
            // A staging failure must not break the create-world screen.
            Agent.diagnose("the mods' data pack could not be staged for a world being created ("
                    + unstaged.getClass().getName() + ")");
        }
    }

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

        // Preserve packs the player disabled when recreating a world.
        Object selection = Members.method(
                type, GameMembers.CREATE_WORLD_SCREEN_GET_DATA_PACK_SELECTION_SETTINGS)
                .invoke(screen, configuration);
        if (selection == null) {
            return;
        }

        int staged = Hook.ravelWriteStagedDatapack(String.valueOf(pairPart(selection, "getFirst")));
        if (staged == UNSTAGED) {
            return;
        }

        Object repository = pairPart(selection, "getSecond");
        switch (select(repository, Hook.ravelPackId())) {
            case REFUSED -> Agent.diagnose(
                    "the game did not find the mods' data pack where it was staged, so this world "
                            + "will have none of their recipes or tags");
            // False suppresses the experimental-content dialog for Ravel's own pack.
            case ADDED -> Members.method(
                    type, GameMembers.CREATE_WORLD_SCREEN_TRY_APPLY_NEW_DATA_PACKS)
                    .invoke(screen, repository, false, reportValidationFailure());
            case ALREADY -> {
                if (staged == STAGED_ANEW) {
                    // The selection is unchanged, but changed files still require a reload.
                    Members.method(type, GameMembers.CREATE_WORLD_SCREEN_APPLY_NEW_PACK_CONFIG)
                            .invoke(screen, repository, configuration, reportValidationFailure());
                }
            }
        }
    }

    private enum Selection {
        REFUSED,
        ADDED,
        ALREADY
    }

    /** Rescans and adds the pack without replacing the player's existing selection. */
    private static Selection select(Object repository, String id)
            throws ReflectiveOperationException {
        Class<?> type = repository.getClass();
        Members.method(type, GameMembers.PACK_REPOSITORY_RELOAD).invoke(repository);
        Collection<?> already = selected(repository);
        if (already.contains(id)) {
            return Selection.ALREADY;
        }
        Set<Object> selection = new LinkedHashSet<>(already);
        // Last is highest priority, so mod files override the player's packs.
        selection.add(id);
        Members.method(type, GameMembers.PACK_REPOSITORY_SET_SELECTED)
                .invoke(repository, selection);
        return selected(repository).contains(id) ? Selection.ADDED : Selection.REFUSED;
    }

    private static Collection<?> selected(Object repository) throws ReflectiveOperationException {
        return (Collection<?>) Members.method(
                repository.getClass(), GameMembers.PACK_REPOSITORY_GET_SELECTED_IDS)
                .invoke(repository);
    }

    /** Resolves a library pair without linking its type into the agent jar. */
    private static Object pairPart(Object pair, String half) throws ReflectiveOperationException {
        Method part = pair.getClass().getMethod(half);
        part.setAccessible(true);
        return part.invoke(pair);
    }

    /** Reports validation refusal when the player returns from the pack-loading screen. */
    private static Consumer<Object> reportValidationFailure() {
        return refused -> Agent.diagnose(
                "the game refused the mods' data pack while creating a world, so this world will "
                        + "have none of their recipes or tags");
    }
}
