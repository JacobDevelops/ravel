package net.minecraft.commands;

/**
 * What a performed command carries as its source.
 *
 * <p>Seeded with no member because nothing calls one: it exists so the pinned
 * {@code performPrefixedCommand} descriptor names a type that resolves, and so
 * the rewritten prologue has a reference to pass as an opaque handle.
 */
public final class CommandSourceStack {
}
