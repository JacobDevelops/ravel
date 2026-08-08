package dev.ravel.jvm;

/**
 * One method the agent rewrites, and the hook it rewrites it to call.
 *
 * <p>The generator emits an array of these into {@link Hook}. A target is
 * matched by name <em>and</em> exact descriptor, because two members of one
 * class can share a name: the reviewed inventory carries two {@code filterText}
 * overloads that are told apart by nothing else.
 *
 * @param internalClass class to rewrite, as the JVM writes its name
 * @param member method to rewrite
 * @param descriptor exact descriptor of the method to rewrite
 * @param isStatic whether the method is declared static
 * @param hookMethod name of the generated hook to call
 * @param hookDescriptor descriptor of the generated hook to call
 * @param refuses whether the hook answers a boolean the prologue returns on,
 *     which the reviewed inventory opts a target into and nothing else can
 */
public record HookTarget(
        String internalClass,
        String member,
        String descriptor,
        boolean isStatic,
        String hookMethod,
        String hookDescriptor,
        boolean refuses) {
}
