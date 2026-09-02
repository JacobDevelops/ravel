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
    private static final MethodTypeDesc TITLE_MENU_COLUMN =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;II)V");
    private static final MethodTypeDesc WORLD_OPEN =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;Ljava/lang/Object;Z)Z");
    private static final MethodTypeDesc WORLD_SAVE =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V");
    private static final MethodTypeDesc REGISTER_CONTENT = MethodTypeDesc.ofDescriptor("()V");
    private static final MethodTypeDesc PERFORM_OUTWARD = MethodTypeDesc.ofDescriptor("()V");

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
     * Adds the mods button under a menu column whose builder returns nothing.
     *
     * <p>The int-returning shape hands back the row its last option took, and
     * the next row follows from it. This shape hands back nothing, and the row
     * cannot be worked out from the column either: a title screen that places
     * its own lower row at a constant reserves nothing when a row is added to
     * the column, so a row derived from the column's base and spacing lands on
     * top of the game's own options. That is what the third real launch drew.
     *
     * <p>So the row is version data, generated into {@link MenuLayout} from the
     * seam's own inventory entry, and this passes it as the row the shared
     * placement sits one spacing below. It may be negative, and at the version
     * that forced this it is: that title screen has no gap below the column wide
     * enough for a row, and the one above it is clear at every size the game
     * scales to.
     */
    public static void addTitleMenuColumn(Object titleScreen, int top, int spacing) {
        Screens.addTitleMenu(titleScreen, top + MenuLayout.RAVEL_ROW - spacing, spacing);
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

    /**
     * Gives the rewritten freeze method a public path to the content adapter.
     *
     * <p>Named here for the same reason as the two above: registering is an
     * action, ADG-4's hooks observe and veto, so the action is a second call at
     * the same seam rather than a widened hook contract. It takes nothing — the
     * set to register is read across the adapter's own crossing, not out of the
     * host method's arguments.
     */
    public static void registerContent() {
        Content.register();
    }

    /**
     * Gives the rewritten client tick a public path to the outward adapter.
     *
     * <p>The same shape as the three above, and the same reason: performing an
     * action is not something ADG-4's hooks do, so it is a second call at a seam
     * the reviewed inventory already pins rather than a widened hook contract.
     * The tick is where it goes because an action is drained rather than pushed,
     * and a tick is the coarsest thing that happens often enough for a player not
     * to hear the delay.
     */
    public static void performOutward() {
        Outward.perform();
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
        // Counted only when a method was actually rewritten. A class whose
        // targets all matched nothing is a class this transformer read and left
        // alone, and counting it made the shutdown report's two figures say a
        // seam was installed where the second figure says none was.
        if (classMethods > 0) {
            transformedClasses.incrementAndGet();
        }
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
        return target.hookMethod().equals("worldOpenPersistence");
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
        return target.hookMethod().equals("worldSavePersistence");
    }

    /**
     * The seam content is registered at.
     *
     * <p>The prologue runs before the body that closes the built-in registries,
     * so this is the last moment they still accept an entry and every vanilla
     * entry is already in them. Registering earlier would put a mod's block in
     * front of the game's own.
     */
    private static boolean isBuiltInFreeze(HookTarget target) {
        return target.hookMethod().equals("registryBuiltInFreeze");
    }

    /**
     * The seam a queued action is performed at.
     *
     * <p>{@code Minecraft} declares more than one tick and the client frame one
     * is a separate hook this must not also fire from. Naming the hook rather
     * than the member tells them apart without a descriptor, because one id
     * renders one hook name.
     */
    private static boolean isClientTick(HookTarget target) {
        return target.hookMethod().equals("clientTick");
    }

    private static boolean isTitleMenu(HookTarget target) {
        return target.hookMethod().equals("screenTitleMenuOptions");
    }

    private static final class PrependTransform implements CodeTransform {
        private final HookTarget target;
        private final MethodTypeDesc targetType;
        private final MethodTypeDesc hookType;
        private final TypeKind returnKind;
        private boolean injected;

        private PrependTransform(HookTarget target) {
            this.target = target;
            targetType = MethodTypeDesc.ofDescriptor(target.descriptor());
            hookType = MethodTypeDesc.ofDescriptor(target.hookDescriptor());
            returnKind = TypeKind.from(targetType.returnType());
            if (isTitleMenu(target)
                    && returnKind != TypeKind.INT
                    && returnKind != TypeKind.VOID) {
                // Refused rather than skipped. A shape neither bridge is written
                // for used to mean the menu was quietly not extended, and this
                // is the class of defect where a silent miss costs more than a
                // recorded failure.
                throw new IllegalArgumentException(
                        "the title-menu target returns " + returnKind
                                + ", and the bridge is written for an int or for nothing");
            }
            if (isWorldOpen(target)) {
                requireWorldOpenShape();
            }
        }

        /**
         * Refuses a world-open target the injected sequence cannot load from.
         *
         * <p>The slots are derived from the descriptor, so a parameter that moved
         * is followed. What cannot be derived is what sits there once it is
         * reached, and the two instructions disagree with a drifted list in
         * opposite ways: an {@code aload} of an {@code int} and an {@code iload}
         * of a reference are both a {@code VerifyError} when the game loads the
         * rewritten class, which is a launch that aborts rather than a build that
         * fails. The scaffold check holds the tracked versions to this shape, and
         * this is the same rule at the one place a version's data cannot reach:
         * the running JVM.
         */
        private void requireWorldOpenShape() {
            int last = targetType.parameterCount() - 1;
            if (last < 2) {
                throw new IllegalArgumentException(
                        "the world-open target declares " + targetType.parameterCount()
                                + " parameters, and the adapter is handed its first, its third"
                                + " and its last");
            }
            requireKind(0, TypeKind.REFERENCE, "the storage access");
            requireKind(2, TypeKind.REFERENCE, "the world stem");
            requireKind(last, TypeKind.BOOLEAN, "the advisory new-world marker");
        }

        /** Refuses one parameter the injected load cannot read as {@code kind}. */
        private void requireKind(int parameter, TypeKind kind, String role) {
            TypeKind declared = TypeKind.from(targetType.parameterList().get(parameter));
            if (declared != kind) {
                throw new IllegalArgumentException(
                        "parameter " + parameter + " of the world-open target is " + declared
                                + " and " + role + " is loaded as " + kind + " there");
            }
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
                    // a refusal has to close, and the advisory new-world marker is
                    // the last parameter the target declares — counted rather than
                    // written down, because the versions differ in how many
                    // parameters sit between the stem and it. A refusal returns
                    // from the host method before its body's first persistent
                    // mutation.
                    builder.aload(slot(0));
                    builder.aload(slot(2));
                    builder.iload(slot(targetType.parameterCount() - 1));
                    builder.invokestatic(TRANSFORMER, "openWorldPersistence", WORLD_OPEN);
                    builder.ifThen(blk -> blk.return_());
                } else if (isWorldSave(target)) {
                    // The receiver, which is the only place the saving world's
                    // identity can come from.
                    builder.aload(0);
                    builder.invokestatic(TRANSFORMER, "saveWorldPersistence", WORLD_SAVE);
                } else if (isBuiltInFreeze(target)) {
                    builder.invokestatic(TRANSFORMER, "registerContent", REGISTER_CONTENT);
                } else if (isClientTick(target)) {
                    builder.invokestatic(TRANSFORMER, "performOutward", PERFORM_OUTWARD);
                }
            }
            // At every return of the declared kind, which is one call per
            // invocation and not one per site: a builder with an early return
            // has two return instructions and runs exactly one of them. Adding
            // the button at the first site instead would leave the column
            // without it on every path that leaves by the other.
            if (isTitleMenu(target)
                    && element instanceof ReturnInstruction instruction
                    && instruction.typeKind() == returnKind) {
                addMenuButton(builder);
            }
            builder.with(element);
        }

        /**
         * Calls the title-menu bridge at the target's own return.
         *
         * <p>Which sequence that is follows the target's declared return kind
         * rather than a version. The int-returning shape leaves the last option's
         * row on the stack and the bridge hands a row back in its place; the void
         * one leaves nothing, so the column's base and spacing are read out of
         * the arguments the descriptor declares and nothing is handed back.
         */
        private void addMenuButton(CodeBuilder builder) {
            builder.aload(0);
            if (returnKind == TypeKind.INT) {
                builder.swap();
                builder.iload(slot(1));
                builder.invokestatic(TRANSFORMER, "addTitleMenu", TITLE_MENU);
                return;
            }
            builder.iload(slot(0));
            builder.iload(slot(1));
            builder.invokestatic(TRANSFORMER, "addTitleMenuColumn", TITLE_MENU_COLUMN);
        }
    }
}
