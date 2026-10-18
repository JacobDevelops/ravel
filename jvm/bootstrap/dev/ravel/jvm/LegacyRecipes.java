package dev.ravel.jvm;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Loads authored legacy crafting JSON; 1.12 has no datapack recipe loader. Bodies go to the
 * game's own parser untranslated, so they must be 1.12.2-native: the type is exactly
 * {@code crafting_shaped} or {@code crafting_shapeless}, never a namespaced 1.13+ id.
 */
final class LegacyRecipes {
    private static final Map<String, Entry> OWNED = new LinkedHashMap<>();
    private static boolean installed;

    private LegacyRecipes() {
    }

    static synchronized void install() throws ReflectiveOperationException {
        if (installed) {
            return;
        }
        List<Entry> staged = new ArrayList<>();
        java.util.Set<String> keys = new java.util.HashSet<>();
        java.util.Set<String> paths;
        try {
            paths = LegacyResources.paths();
        } catch (java.io.IOException failure) {
            throw new IllegalStateException("cannot capture admitted legacy recipe resources", failure);
        }
        for (String path : paths.stream().sorted().toList()) {
            String key = key(path);
            if (key == null) {
                continue;
            }
            if (!keys.add(key)) {
                throw new IllegalStateException("multiple resource paths claim legacy recipe: " + key);
            }
            Object identifier = identifier(key);
            if (LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "getRecipe", null, identifier) != null) {
                throw new IllegalStateException("legacy recipe key already registered: " + key);
            }
            staged.add(new Entry(key, parseResource(path, key)));
        }
        // Every source is decoded and checked before the first vanilla registry mutation.
        for (Entry entry : staged) {
            LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "register", null,
                    identifier(entry.key()), entry.recipe());
            OWNED.put(entry.key(), entry);
        }
        // LaunchContext's admitted supply is immutable; subsequent reloads reuse these identities.
        installed = true;
    }

    static String key(String path) {
        int root = path.startsWith("data/") ? 5 : path.startsWith("assets/") ? 7 : -1;
        if (root < 0 || !path.endsWith(".json")) {
            return null;
        }
        int namespaceEnd = path.indexOf('/', root);
        if (namespaceEnd <= root || !path.startsWith("/recipes/", namespaceEnd)) {
            return null;
        }
        String namespace = path.substring(root, namespaceEnd);
        String name = path.substring(namespaceEnd + 9, path.length() - 5);
        if (!namespace.matches("[a-z0-9_.-]+") || !name.matches("[a-z0-9_./-]+")
                || name.contains("..")) {
            throw new IllegalArgumentException("invalid legacy recipe path: " + path);
        }
        return namespace + ":" + name;
    }

    private static Object identifier(String key) throws ReflectiveOperationException {
        int colon = key.indexOf(':');
        return LegacyAccess.create("LEGACY_IDENTIFIER", key.substring(0, colon), key.substring(colon + 1));
    }

    private static Object parse(byte[] bytes) throws ReflectiveOperationException {
        Class<?> parser = Members.game("com.google.gson.JsonParser");
        Object element = parser.getMethod("parse", String.class).invoke(
                parser.getConstructor().newInstance(), new String(bytes, StandardCharsets.UTF_8));
        Object json = Members.game("com.google.gson.JsonElement").getMethod("getAsJsonObject").invoke(element);
        return LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "parseRecipe", null, json);
    }

    private static Object parseResource(String path, String key) {
        try {
            return parse(LegacyResources.bytes(path));
        } catch (ReflectiveOperationException | java.io.IOException | RuntimeException failure) {
            throw new IllegalStateException("invalid admitted legacy recipe " + key
                    + " from " + path, failure);
        }
    }

    static synchronized Map<String, Object> installed() {
        Map<String, Object> recipes = new LinkedHashMap<>();
        OWNED.forEach((key, entry) -> recipes.put(key, entry.recipe()));
        return Map.copyOf(recipes);
    }

    private record Entry(String key, Object recipe) {
    }
}
