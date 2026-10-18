package dev.ravel.legacy.launch;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

/** Runtime remaps use the same owner inventory resource as the standalone surface scanner. */
public final class LegacyAdapters {
    private static final Map<String, String> OWNERS = readOwners();

    private LegacyAdapters() {}

    private static Map<String, String> readOwners() {
        var resource = LegacyAdapters.class.getResourceAsStream("/dev/ravel/legacy/adapter-owners.txt");
        if (resource == null) throw new IllegalStateException("Missing legacy adapter owner resource");
        try (var reader = new BufferedReader(new InputStreamReader(resource, StandardCharsets.UTF_8))) {
            var owners = new LinkedHashMap<String, String>();
            for (String suffix : reader.lines().toList()) {
                if (!suffix.isBlank()) owners.put("org/lwjgl/" + suffix, "dev/ravel/legacy/" + suffix);
            }
            return Map.copyOf(owners);
        } catch (IOException failure) {
            throw new ExceptionInInitializerError(failure);
        }
    }

    /** Maps a slash-separated owner or every object type in a JVM descriptor. */
    public static String map(String value) {
        String exact = OWNERS.get(value);
        if (exact != null) return exact;
        String result = value;
        for (var entry : OWNERS.entrySet()) {
            result = result.replace("L" + entry.getKey() + ";", "L" + entry.getValue() + ";");
        }
        return result;
    }
}
