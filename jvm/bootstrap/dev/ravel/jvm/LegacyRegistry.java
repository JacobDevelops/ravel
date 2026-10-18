package dev.ravel.jvm;

import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;

/** Preserves the three independent identities of a legacy registry entry. */
final class LegacyRegistry {
    private final Map<Integer, Entry> numbers = new HashMap<>();
    private final Map<String, Entry> keys = new HashMap<>();
    private final Map<Object, Entry> values = new IdentityHashMap<>();
    private final int maximum;
    private int next;

    LegacyRegistry(int maximum) {
        this.maximum = maximum;
    }

    synchronized void check(int number, String key, Object value) {
        if (number < 0 || number > maximum || key == null || value == null) {
            throw new IllegalStateException("invalid legacy registration " + number + "/" + key);
        }
        Entry entry = new Entry(number, key, value);
        compatible(numbers.get(number), entry);
        compatible(keys.get(key), entry);
        compatible(values.get(value), entry);
    }

    synchronized void record(int number, String key, Object value) {
        check(number, key, value);
        Entry entry = new Entry(number, key, value);
        numbers.put(number, entry);
        keys.put(key, entry);
        values.put(value, entry);
        next = Math.max(next, number + 1);
    }

    synchronized int nextId() {
        if (next > maximum) {
            throw new IllegalStateException("legacy numeric registry exhausted");
        }
        return next;
    }

    synchronized int capacity() {
        return next;
    }

    synchronized int size() {
        return numbers.size();
    }

    synchronized Object value(String key) {
        Entry entry = keys.get(key);
        return entry == null ? null : entry.value();
    }

    synchronized int number(String key) {
        Entry entry = keys.get(key);
        if (entry == null) {
            throw new IllegalStateException("unknown legacy registry identity: " + key);
        }
        return entry.number();
    }

    private static void compatible(Entry old, Entry entry) {
        if (old != null && (old.number() != entry.number()
                || !old.key().equals(entry.key()) || old.value() != entry.value())) {
            throw new IllegalStateException("conflicting legacy registry identity: " + entry.key());
        }
    }

    private record Entry(int number, String key, Object value) {
    }
}
