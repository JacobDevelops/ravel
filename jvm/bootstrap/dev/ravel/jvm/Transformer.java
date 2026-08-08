package dev.ravel.jvm;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.classfile.MethodTransform;
import java.lang.classfile.TypeKind;
import java.lang.classfile.instruction.ReturnInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
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
 *
 * <p>A target the reviewed inventory opts in is the one exception, on ADG-1's
 * Phase C amendment: its hook answers a boolean and the prologue returns from the
 * host method when it is true. The opt-in arrives as generated data on
 * {@link HookTarget}, so nothing written here can widen the set, and only a void
 * target may carry it, so the early return invents no value.
 */
public final class Transformer implements ClassFileTransformer {

    private static final ClassDesc HOOK = ClassDesc.of("dev.ravel.jvm.Hook");
    private static final ClassDesc TRANSFORMER = ClassDesc.of("dev.ravel.jvm.Transformer");
    private static final MethodTypeDesc TITLE_MENU =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;II)I");
    private static final MethodTypeDesc WORLD_OPEN =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;Ljava/lang/Object;Z)Z");
    private static final MethodTypeDesc WORLD_SAVE =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V");

    private final Map<String, List<HookTarget>> byClass;
    private final AtomicInteger transformedClasses = new AtomicInteger();
    private final AtomicInteger rewrittenMethods = new AtomicInteger();
    private final ConcurrentMap<String, Integer> rewrittenByClass;
    private final Set<String> installed = ConcurrentHashMap.newKeySet();
    private final ConcurrentMap<String, String> failedByClass = new ConcurrentHashMap<>();
    private final AtomicReference<String> loaderIdentity = new AtomicReference<>();

    /**
     * Indexes the generated targets by the class each one lives in.
     *
     * @param targets every target the generator emitted
     * @param collectTargets whether to retain per-class evidence
     */
    public Transformer(HookTarget[] targets, boolean collectTargets) {
        Map<String, List<HookTarget>> index = new HashMap<>();
        for (HookTarget target : targets) {
            index.computeIfAbsent(target.internalClass(), key -> new ArrayList<>()).add(target);
        }
        this.byClass = Map.copyOf(index);
        this.rewrittenByClass = collectTargets ? new ConcurrentHashMap<>() : null;
    }

    /** Gives transformed game code a public path to the package-private screen loader. */
    public static int addTitleMenu(Object titleScreen, int bottom, int spacing) {
        return Screens.addTitleMenu(titleScreen, bottom, spacing);
    }

    /**
     * Gives the rewritten world-load method a public path to ADG-7's adapter.
     *
     * @return whether to return from the host method before its body runs
     */
    public static boolean openWorldPersistence(Object access, Object stem, boolean freshWorld) {
        return Persistence.open(access, stem, freshWorld);
    }

    /** Gives the rewritten save method a public path to ADG-7's adapter. */
    public static void saveWorldPersistence(Object server) {
        Persistence.save(server);
    }

    /** Classes this transformer rewrote. */
    public int transformedClasses() {
        return transformedClasses.get();
    }

    /** Methods this transformer rewrote. */
    public int rewrittenMethods() {
        return rewrittenMethods.get();
    }

    /** Reports each class whose rewrite failed and the throwable type that stopped it. */
    public String transformationFailures() {
        Map<String, String> sorted = new TreeMap<>(failedByClass);
        if (sorted.isEmpty()) {
            return "";
        }
        StringBuilder report = new StringBuilder("[ravel] failed to transform ")
                .append(sorted.size())
                .append(" classes\n");
        for (Map.Entry<String, String> failure : sorted.entrySet()) {
            report.append("[ravel] failed target: ")
                    .append(failure.getKey())
                    .append('=')
                    .append(failure.getValue())
                    .append('\n');
        }
        return report.toString();
    }

    /**
     * Whether the target that calls {@code hookMethod} was actually rewritten.
     *
     * <p>ADG-7's installed-prologue marker. It is separate state from the
     * counters and the reported target table above, and deliberately so: those
     * are diagnostics that {@code -Dravel.report.targets} turns on, and a marker
     * a launch flag can turn off is not a marker. This set is collected
     * unconditionally and its only reader is the world-open path.
     *
     * <p>A target enters it only once a prepend matched the method, which is what
     * tells a class that merely loaded apart from a seam that was actually
     * installed. That distinction is the hole ADG-7's rejected alternative —
     * checking only for a jar or a class — leaves open, because a transformation
     * can return the original class or match zero methods and still leave
     * everything present.
     *
     * @param hookMethod the generated hook name the target is rewritten to call
     */
    public boolean isInstalled(String hookMethod) {
        return installed.contains(hookMethod);
    }

    /** Names each rewritten class and the methods matched inside it, or {@code null} when disabled. */
    public String rewrittenTargets() {
        if (rewrittenByClass == null) {
            return null;
        }
        Map<String, Integer> sorted = new TreeMap<>(rewrittenByClass);
        StringBuilder report = new StringBuilder();
        for (Map.Entry<String, Integer> entry : sorted.entrySet()) {
            if (!report.isEmpty()) {
                report.append(',');
            }
            report.append(entry.getKey()).append('=').append(entry.getValue());
        }
        return report.toString();
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

        ClassLoader resources = loader == null ? ClassLoader.getSystemClassLoader() : loader;
        ClassFile context = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                ClassHierarchyResolver.ofResourceParsing(resources)));
        int classMethods = 0;
        // Held aside rather than published as each target matches. A later
        // target in the same class can throw, and this method then returns the
        // original bytes — so a marker already published would name a seam that
        // is not in the class the JVM went on to load, which is precisely the
        // fail-open answer the marker exists to prevent.
        List<String> matched = new ArrayList<>();
        byte[] rewritten = classfile;
        for (HookTarget target : targets) {
            // One pass per target rather than one predicate over all of them:
            // the code transform has to know which target it is injecting, and a
            // handful of passes over one class is not a cost anyone can measure.
            PrependTransform prepend;
            try {
                prepend = new PrependTransform(target);
                rewritten =
                        context.transformClass(
                                context.parse(rewritten),
                                ClassTransform.transformingMethods(
                                        model ->
                                                model.methodName().equalsString(target.member())
                                                        && model.methodType()
                                                                .equalsString(target.descriptor()),
                                        MethodTransform.transformingCode(prepend)));
            } catch (Throwable failure) {
                String dottedName = internalName.replace('/', '.');
                String failureType = failure.getClass().getName();
                failedByClass.put(dottedName, failureType);
                Agent.diagnose("failed to transform " + dottedName + " (" + failureType + ")");
                return null;
            }
            if (prepend.matched()) {
                matched.add(target.hookMethod());
                classMethods++;
            }
        }
        transformedClasses.incrementAndGet();
        rewrittenMethods.addAndGet(classMethods);
        // Published only now, when the rewritten class is the one being returned.
        installed.addAll(matched);
        if (rewrittenByClass != null && classMethods > 0) {
            rewrittenByClass.put(internalName.replace('/', '.'), classMethods);
        }
        return rewritten;
    }

    /**
     * ADG-7's open seam, which carries the version adapter rather than a hook.
     *
     * <p>Named here the way the title-menu bridge is named here: the generated
     * hook takes what ADG-4 admits and returns a verdict, and the adapter needs
     * two of the method's own arguments to read the world and to clean it up
     * again. That is version-specific translation rather than a hook, so it is a
     * second call at the same seam instead of a widened hook contract.
     */
    private static boolean isWorldOpen(HookTarget target) {
        return target.internalClass().equals("net/minecraft/client/Minecraft")
                && target.member().equals("doWorldLoad");
    }

    /**
     * ADG-7's save seam.
     *
     * <p>This one needs the receiver, which no hook is given: the saving world's
     * identity is {@code MinecraftServer.getWorldPath(ROOT)} and there is nowhere
     * else it can come from. ADG-4 keeps that out of a hook's hands, so it is
     * handed to the adapter instead, exactly as the title-menu bridge takes the
     * screen it adds a button to.
     */
    private static boolean isWorldSave(HookTarget target) {
        return target.internalClass().equals("net/minecraft/server/MinecraftServer")
                && target.member().equals("saveAllChunks");
    }

    private static boolean isTitleMenu(HookTarget target) {
        return target.internalClass().equals("net/minecraft/client/gui/screens/TitleScreen")
                && target.member().equals("createNormalMenuOptions")
                && target.descriptor().equals("(II)I");
    }

    private static final class PrependTransform implements CodeTransform {
        private final HookTarget target;
        private final MethodTypeDesc targetType;
        private final MethodTypeDesc hookType;
        private boolean injected;

        private PrependTransform(HookTarget target) {
            this.target = target;
            targetType = MethodTypeDesc.ofDescriptor(target.descriptor());
            hookType = MethodTypeDesc.ofDescriptor(target.hookDescriptor());
        }

        private boolean matched() {
            return injected;
        }

        /**
         * The local slot one declared parameter arrived in.
         *
         * <p>Counted rather than written down, because a {@code long} or a
         * {@code double} earlier in the list takes two slots and an index would
         * be wrong from that point on.
         */
        private int slot(int parameter) {
            int at = target.isStatic() ? 0 : 1;
            for (int index = 0; index < parameter; index++) {
                at += TypeKind.from(targetType.parameterList().get(index)).slotSize();
            }
            return at;
        }

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
                if (target.refuses()) {
                    // The hook left a boolean on the stack and `ifThen` branches
                    // on IFNE, so a true verdict returns from the host method
                    // before its body runs. Only a void target may opt in, so
                    // this is a bare RETURN and no value is ever invented.
                    builder.ifThen(blk -> blk.return_());
                }
                // After the hook and its verdict, never before: the generated
                // hook's boolean is on the stack until the branch above consumes
                // it, and an observer that refused has already left the method.
                if (isWorldOpen(target)) {
                    // The storage access and the world stem are the two arguments
                    // a refusal has to close, and the last one is the advisory
                    // new-world marker. A refusal returns from the host method
                    // before its body's first persistent mutation.
                    builder.aload(slot(0));
                    builder.aload(slot(2));
                    builder.iload(slot(4));
                    builder.invokestatic(TRANSFORMER, "openWorldPersistence", WORLD_OPEN);
                    builder.ifThen(blk -> blk.return_());
                } else if (isWorldSave(target)) {
                    // The receiver, which is the only place the saving world's
                    // identity can come from.
                    builder.aload(0);
                    builder.invokestatic(TRANSFORMER, "saveWorldPersistence", WORLD_SAVE);
                }
            }
            if (isTitleMenu(target)
                    && element instanceof ReturnInstruction instruction
                    && instruction.typeKind() == TypeKind.INT) {
                builder.aload(0);
                builder.swap();
                builder.iload(2);
                builder.invokestatic(TRANSFORMER, "addTitleMenu", TITLE_MENU);
            }
            builder.with(element);
        }
    }
}
