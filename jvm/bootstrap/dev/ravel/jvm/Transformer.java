package dev.ravel.jvm;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.classfile.MethodTransform;
import java.lang.classfile.TypeKind;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Rewrites every reviewed target to call its generated hook first.
 *
 * <p>The rewrite uses the runtime's own {@code java.lang.classfile}, so no
 * bytecode library ships with Ravel and none has to be version-matched to the
 * runtime on a game update.
 *
 * <p>The injected sequence loads the target's own arguments and calls one static
 * hook that returns nothing. It passes the declared arguments and never the
 * receiver: ADG-4 decided the game keeps its state, so a hook sees what the call
 * site was given rather than the object holding it. A reference among those
 * arguments is passed as it stands, and the generated hook declares it as
 * {@code Object}, which is the opaque handle that gate's M3-W0 amendment admits.
 * A hook that has to stop the call throws, which unwinds the target without any
 * branch here.
 */
public final class Transformer implements ClassFileTransformer {

    private static final ClassDesc HOOK = ClassDesc.of("dev.ravel.jvm.Hook");

    private final Map<String, List<HookTarget>> byClass;
    private final AtomicInteger transformedClasses = new AtomicInteger();
    private final AtomicInteger rewrittenMethods = new AtomicInteger();
    private final AtomicReference<String> loaderIdentity = new AtomicReference<>();

    /**
     * Indexes the generated targets by the class each one lives in.
     *
     * @param targets every target the generator emitted
     */
    public Transformer(HookTarget[] targets) {
        Map<String, List<HookTarget>> index = new HashMap<>();
        for (HookTarget target : targets) {
            index.computeIfAbsent(target.internalClass(), key -> new ArrayList<>()).add(target);
        }
        this.byClass = Map.copyOf(index);
    }

    /** Classes this transformer rewrote. */
    public int transformedClasses() {
        return transformedClasses.get();
    }

    /** Methods this transformer rewrote. */
    public int rewrittenMethods() {
        return rewrittenMethods.get();
    }

    /**
     * Describes the loader that loaded the first class this transformer
     * rewrote, or {@code null} if it rewrote nothing.
     *
     * <p>ADG-1's second candidate turns on which loader resolves the game's
     * classes, and {@link #transform} received that loader and discarded it, so
     * nothing had ever measured it. The description is built at the moment of
     * the rewrite and kept as text rather than as a reference, so recording it
     * holds nothing of the game alive.
     */
    public String loaderIdentity() {
        return loaderIdentity.get();
    }

    /**
     * Names one loader as its class and its identity hash, so two entries in a
     * chain can be told apart even when they are the same class.
     */
    private static String name(ClassLoader loader) {
        return loader == null
                ? "bootstrap"
                : loader.getClass().getName() + "@"
                        + Integer.toHexString(System.identityHashCode(loader));
    }

    /** Describes {@code loader} against the system loader and its parents. */
    private static String describe(ClassLoader loader) {
        ClassLoader system = ClassLoader.getSystemClassLoader();
        StringBuilder chain = new StringBuilder();
        for (ClassLoader current = loader; current != null; current = current.getParent()) {
            chain.append(name(current)).append(" -> ");
        }
        chain.append(name(null));

        return "rewrittenBy=" + name(loader)
                + " system=" + name(system)
                + " sameObject=" + (loader == system)
                + " parentChain=" + chain;
    }

    @Override
    public byte[] transform(
            Module module,
            ClassLoader loader,
            String internalName,
            Class<?> beingRedefined,
            ProtectionDomain domain,
            byte[] classfile) {
        if (internalName == null) {
            return null;
        }
        List<HookTarget> targets = byClass.get(internalName);
        if (targets == null) {
            return null;
        }
        // Recorded before the rewrite rather than after, so a rewrite that
        // throws still leaves the identity of the loader that asked for it.
        loaderIdentity.compareAndSet(null, describe(loader));

        ClassFile context = ClassFile.of();
        byte[] rewritten = classfile;
        for (HookTarget target : targets) {
            // One pass per target rather than one predicate over all of them:
            // the code transform has to know which target it is injecting, and a
            // handful of passes over one class is not a cost anyone can measure.
            rewritten =
                    context.transformClass(
                            context.parse(rewritten),
                            ClassTransform.transformingMethods(
                                    model ->
                                            model.methodName().equalsString(target.member())
                                                    && model.methodType()
                                                            .equalsString(target.descriptor()),
                                    MethodTransform.transformingCode(prepend(target))));
        }
        transformedClasses.incrementAndGet();
        return rewritten;
    }

    private CodeTransform prepend(HookTarget target) {
        MethodTypeDesc targetType = MethodTypeDesc.ofDescriptor(target.descriptor());
        MethodTypeDesc hookType = MethodTypeDesc.ofDescriptor(target.hookDescriptor());
        return new CodeTransform() {
            private boolean injected;

            @Override
            public void accept(CodeBuilder builder, CodeElement element) {
                if (!injected) {
                    injected = true;
                    // Local slot zero is the receiver on an instance method, and
                    // the first argument on a static one.
                    int slot = target.isStatic() ? 0 : 1;
                    for (ClassDesc parameter : targetType.parameterList()) {
                        TypeKind kind = TypeKind.from(parameter);
                        builder.loadLocal(kind, slot);
                        slot += kind.slotSize();
                    }
                    builder.invokestatic(HOOK, target.hookMethod(), hookType);
                    rewrittenMethods.incrementAndGet();
                }
                builder.with(element);
            }
        };
    }
}
