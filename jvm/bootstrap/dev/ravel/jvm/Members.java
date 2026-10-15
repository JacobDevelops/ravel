package dev.ravel.jvm;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

/**
 * Finding one game member from the name and descriptor its version links it as.
 *
 * <p>{@link GameNames} says what a class role links to and {@link GameMembers}
 * says what a member role links to; this is what turns the second into a member
 * the JVM will let an adapter call. Both generated classes come from the
 * version's reviewed reflective surface, so no source under {@code jvm/bootstrap}
 * spells a game member and one shared tree stays right about every version.
 *
 * <p><b>Why a descriptor and not an argument count.</b> A name and a width
 * identifies a member only where names are unique enough to. Under an obfuscated
 * version they are not: 1.21.1 links {@code DimensionDataStorage.computeIfAbsent}
 * and {@code set} both as a two-argument {@code a}, and {@code get} shares
 * {@code b} with a two-argument lambda. A lookup by width would take whichever
 * the class file declared first, potentially reading a world with the method
 * that writes one.
 *
 * <p>Descriptors are built from the reflected member rather than read off it,
 * because the JVM exposes no descriptor on {@link Method}. What is built here is
 * the erased form the class file carries, which is what the generator emitted.
 */
final class Members {

    /** What separates a linked name from its linked descriptor. */
    private static final char SEPARATOR = ':';

    private Members() {
    }

    /**
     * One game class, resolved through the loader the game itself is on.
     *
     * <p>The one place in the agent jar a game name becomes a class. Which
     * loader that is was the second half of what the first real dedicated server
     * found: the client's game is on the system loader, and the published
     * server's is only inside the child loader its bundler builds, so a lookup
     * that named the system loader answered {@code ClassNotFoundException} for
     * every class the world persistence and content adapters reach.
     *
     * @param name the class as this version's names spell it
     */
    static Class<?> game(String name) throws ReflectiveOperationException {
        return Class.forName(name, true, Agent.gameLoader());
    }

    /**
     * The method one generated constant names, on the class or any supertype.
     *
     * @param owner where to start looking, usually a receiver's own class
     * @param member one {@link GameMembers} constant
     * @throws NoSuchMethodException when nothing declares that exact member
     */
    static Method method(Class<?> owner, String member) throws ReflectiveOperationException {
        String name = name(member);
        String descriptor = descriptor(member);
        for (Class<?> current = owner; current != null; current = current.getSuperclass()) {
            for (Method candidate : current.getDeclaredMethods()) {
                if (candidate.getName().equals(name) && descriptor.equals(descriptorOf(candidate))) {
                    candidate.setAccessible(true);
                    return candidate;
                }
            }
        }
        throw absent(owner, member);
    }

    /**
     * The static field one generated constant names.
     *
     * @throws NoSuchFieldException when nothing declares that exact member
     */
    static Field field(Class<?> owner, String member) throws ReflectiveOperationException {
        String name = name(member);
        String descriptor = descriptor(member);
        for (Field candidate : owner.getDeclaredFields()) {
            if (candidate.getName().equals(name)
                    && descriptor.equals(descriptorOf(candidate.getType()))) {
                candidate.setAccessible(true);
                return candidate;
            }
        }
        throw new NoSuchFieldException(owner.getName() + "." + member);
    }

    /**
     * The constructor one generated constant names.
     *
     * @throws NoSuchMethodException when nothing declares that exact member
     */
    static Constructor<?> constructor(Class<?> owner, String member)
            throws ReflectiveOperationException {
        String descriptor = descriptor(member);
        for (Constructor<?> candidate : owner.getDeclaredConstructors()) {
            if (descriptor.equals(descriptorOf(candidate.getParameterTypes(), void.class))) {
                candidate.setAccessible(true);
                return candidate;
            }
        }
        throw absent(owner, member);
    }

    /** The linked name half of a generated constant. */
    private static String name(String member) {
        return member.substring(0, member.indexOf(SEPARATOR));
    }

    /** The linked descriptor half of a generated constant. */
    private static String descriptor(String member) {
        return member.substring(member.indexOf(SEPARATOR) + 1);
    }

    /** The erased descriptor the class file carries for one reflected method. */
    private static String descriptorOf(Method method) {
        return descriptorOf(method.getParameterTypes(), method.getReturnType());
    }

    /** The erased descriptor of one argument list and return type. */
    private static String descriptorOf(Class<?>[] parameters, Class<?> returned) {
        StringBuilder text = new StringBuilder(parameters.length * 24 + 16);
        text.append('(');
        for (Class<?> parameter : parameters) {
            text.append(descriptorOf(parameter));
        }
        text.append(')');
        return text.append(descriptorOf(returned)).toString();
    }

    /**
     * The descriptor of one type.
     *
     * <p>An array's binary name is already its descriptor once the package
     * separators are the ones a class file writes, which is why only the
     * primitives and the plain reference case are spelled out.
     */
    private static String descriptorOf(Class<?> type) {
        String name = type.getName();
        if (name.charAt(0) == '[') {
            return name.replace('.', '/');
        }
        return switch (name) {
            case "boolean" -> "Z";
            case "byte" -> "B";
            case "char" -> "C";
            case "short" -> "S";
            case "int" -> "I";
            case "long" -> "J";
            case "float" -> "F";
            case "double" -> "D";
            case "void" -> "V";
            default -> "L" + name.replace('.', '/') + ";";
        };
    }

    /**
     * One static entry point of the sibling artifact, named as {@code name/arity}.
     *
     * <p>Name and arity is enough here and nowhere else, because these are
     * Ravel's own names on Ravel's own generated class: nothing renames them and
     * nothing overloads them. Every game member goes through the descriptor its
     * version links instead.
     *
     * <p>The entry points are declared as constants beside the calls that use
     * them so the per-version Java compile can require each version's sibling to
     * carry them. The sibling is generated per version and only one version is
     * built at a time, so a version whose override dropped an entry point would
     * otherwise compile, ship, and fail at the first mod that needed it.
     *
     * @param owner the sibling class
     * @param entry the entry point, as its name, a slash, and its argument count
     * @throws NoSuchMethodException when the sibling declares no such entry point
     */
    static Method sibling(Class<?> owner, String entry) throws ReflectiveOperationException {
        int slash = entry.lastIndexOf('/');
        String name = entry.substring(0, slash);
        int arity = Integer.parseInt(entry.substring(slash + 1));
        for (Method candidate : owner.getDeclaredMethods()) {
            if (candidate.getName().equals(name) && candidate.getParameterCount() == arity) {
                candidate.setAccessible(true);
                return candidate;
            }
        }
        throw absent(owner, entry);
    }

    /** The refusal a missing member lands on, naming what was looked for. */
    private static NoSuchMethodException absent(Class<?> owner, String member) {
        return new NoSuchMethodException(owner.getName() + "." + member);
    }
}
