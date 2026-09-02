package net.minecraft.core;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.resources.Identifier;

/**
 * A stand-in registry with the behaviour that decides whether registration
 * worked: it refuses a duplicate and refuses everything once frozen.
 */
public class Registry {
    private final String name;
    private final Map<String, Object> entries = new LinkedHashMap<>();
    private boolean frozen;

    public Registry(String name) {
        this.name = name;
    }

    public static Object register(Registry registry, Identifier identifier, Object value) {
        if (registry.frozen) {
            throw new IllegalStateException("registry " + registry.name + " is frozen");
        }
        if (registry.entries.putIfAbsent(identifier.toString(), value) != null) {
            throw new IllegalStateException(identifier + " is already registered");
        }
        return value;
    }

    public void freeze() {
        frozen = true;
    }

    /** One registered value, as its own type describes itself. */
    private static String describe(Object value) {
        if (value instanceof net.minecraft.world.level.block.Block block) {
            return "Block(" + block.applied() + ")";
        }
        if (value instanceof net.minecraft.world.item.Item item) {
            return "Item(" + item.applied() + ")";
        }
        return value.getClass().getSimpleName();
    }

    /** What this registry holds, as the harness reads it back. */
    public String contents() {
        StringBuilder read = new StringBuilder(name).append("=");
        boolean first = true;
        for (Map.Entry<String, Object> entry : entries.entrySet()) {
            if (!first) {
                read.append(",");
            }
            first = false;
            read.append(entry.getKey()).append("->").append(describe(entry.getValue()));
        }
        return read.toString();
    }
}
