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
 * Prepends generated hooks using the runtime classfile API.
 * Hooks receive declared arguments, not receivers; refusing hooks may return from void targets.
 */
public final class Transformer implements ClassFileTransformer {

    private static final ClassDesc AGENT = ClassDesc.of("dev.ravel.jvm.Agent");
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
    private static final MethodTypeDesc WORLD_PACK_REPOSITORY =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V");
    private static final MethodTypeDesc CREATE_WORLD_SCREEN =
            MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V");
    private static final MethodTypeDesc REGISTER_CONTENT = MethodTypeDesc.ofDescriptor("()V");
    private static final MethodTypeDesc PERFORM_OUTWARD = MethodTypeDesc.ofDescriptor("()V");

    private static final String ENTRY_MEMBER = "main";

    private static final String MISSING_ENTRY_METHOD = "missing entry method";

    private static final MethodTypeDesc ENTRY_DESCRIPTOR =
            MethodTypeDesc.ofDescriptor("([Ljava/lang/String;)V");

    private static final MethodTypeDesc ENTERED =
            MethodTypeDesc.ofDescriptor("(ILjava/lang/Class;)V");

    private final Map<String, List<HookTarget>> byClass;
    private final AtomicInteger transformedClasses = new AtomicInteger();
    private final AtomicInteger rewrittenMethods = new AtomicInteger();
    private final ConcurrentMap<String, Integer> rewrittenByClass;
    private final Set<String> installed = ConcurrentHashMap.newKeySet();
    private final ConcurrentMap<String, String> failedByClass = new ConcurrentHashMap<>();
    private final AtomicReference<String> loaderIdentity = new AtomicReference<>();

    /** Indexes targets by class and optionally collects per-class rewrite evidence. */
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

    /** Uses the version's row offset for title-menu builders that return no row. */
    public static void addTitleMenuColumn(Object titleScreen, int top, int spacing) {
        Screens.addTitleMenu(titleScreen, top + MenuLayout.RAVEL_ROW - spacing, spacing);
    }

    /** Exposes the pack writer to transformed game code. */
    public static void openWorldDatapack(Object access) {
        Datapack.write(access);
    }

    /** Exposes create-world pack staging to transformed game code. */
    public static void createWorldScreen(Object screen) {
        Datapack.stage(screen);
    }

    /** Returns whether the world-load body must be skipped. */
    public static boolean openWorldPersistence(Object access, Object stem, boolean freshWorld) {
        return Persistence.open(access, stem, freshWorld);
    }

    /** Gives the rewritten save method a public path to the persistence adapter. */
    public static void saveWorldPersistence(Object server) {
        Persistence.save(server);
    }

    /** Registers admitted content after the hook, before built-in registries freeze. */
    public static void registerContent() {
        Content.register();
    }

    /** Drains outward actions after the client-tick hook. */
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

    /** Whether a matching prologue was returned to the JVM, independent of evidence collection. */
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

    /** Records the first rewritten class's loader as text so evidence retains no loader. */
    public String loaderIdentity() {
        return loaderIdentity.get();
    }

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
        // Entry points need initialization prologues even when they are not hook targets.
        int side = Agent.sideOf(internalName);
        byte[] entered = null;
        if (side != Agent.UNNAMED) {
            Agent.sawEntryPoint(internalName);
            entered = enterAt(side, internalName, loader, classfile);
            // An entry point may also carry ordinary hooks.
            classfile = entered;
        }
        List<HookTarget> targets = byClass.get(internalName);
        if (targets == null) {
            return entered;
        }
        // Recorded before the rewrite rather than after, so a rewrite that
        // throws still leaves the identity of the loader that asked for it.
        loaderIdentity.compareAndSet(null, describe(loader));

        ClassLoader resources = loader == null ? ClassLoader.getSystemClassLoader() : loader;
        ClassFile context = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                ClassHierarchyResolver.ofResourceParsing(resources)));
        // Publish markers only after every rewrite succeeds; failure discards partial hook rewrites.
        List<String> matched = new ArrayList<>();
        byte[] rewritten = classfile;
        for (HookTarget target : targets) {
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
                return entered;
            }
            if (prepend.matched()) {
                matched.add(target.hookMethod());
            }
        }
        if (!matched.isEmpty()) {
            transformedClasses.incrementAndGet();
        }
        rewrittenMethods.addAndGet(matched.size());
        installed.addAll(matched);
        if (rewrittenByClass != null && !matched.isEmpty()) {
            rewrittenByClass.put(internalName.replace('/', '.'), matched.size());
        }
        return rewritten;
    }

    /**
     * Defers initialization to the entry-point body so startup classes still reach the transformer.
     * Entry-point rewrites are not counted as inventory hooks.
     */
    private byte[] enterAt(int side, String internalName, ClassLoader loader, byte[] classfile) {
        ClassLoader resources = loader == null ? ClassLoader.getSystemClassLoader() : loader;
        ClassFile context = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                ClassHierarchyResolver.ofResourceParsing(resources)));
        EnterTransform prologue =
                new EnterTransform(side, ClassDesc.ofInternalName(internalName));
        try {
            byte[] rewritten = context.transformClass(
                    context.parse(classfile),
                    ClassTransform.transformingMethods(
                            model -> model.methodName().equalsString(ENTRY_MEMBER)
                                    && model.methodType().equalsString(
                                            ENTRY_DESCRIPTOR.descriptorString()),
                            MethodTransform.transformingCode(prologue)));
            if (!prologue.injected) {
                failedByClass.put(internalName.replace('/', '.'), MISSING_ENTRY_METHOD);
                return classfile;
            }
            return rewritten;
        } catch (Throwable failure) {
            failedByClass.put(internalName.replace('/', '.'), failure.getClass().getName());
            return classfile;
        }
    }

    /** The world-open adapter reads arguments rather than the receiver. */
    private static boolean isWorldOpen(HookTarget target) {
        return target.hookMethod().equals("worldOpenPersistence");
    }

    /** The world-save adapter needs the receiver to resolve the world path. */
    private static boolean isWorldSave(HookTarget target) {
        return target.hookMethod().equals("worldSavePersistence");
    }

    /** Runs after vanilla registration but before built-in registries freeze. */
    private static boolean isBuiltInFreeze(HookTarget target) {
        return target.hookMethod().equals("registryBuiltInFreeze");
    }

    /** The client tick, distinct from the client frame tick. */
    private static boolean isClientTick(HookTarget target) {
        return target.hookMethod().equals("clientTick");
    }

    /** Writes the world's pack before its repository is built. */
    private static boolean isWorldPackRepository(HookTarget target) {
        return target.hookMethod().equals("worldCreatePackRepository");
    }

    /** Stages data before the create-world screen loads what the new world will inherit. */
    private static boolean isCreateWorldScreen(HookTarget target) {
        return target.hookMethod().equals("screenCreateWorldInit");
    }

    private static boolean isTitleMenu(HookTarget target) {
        return target.hookMethod().equals("screenTitleMenuOptions");
    }

    /** Calls {@link Agent#entered} once, before the entry point's own first statement. */
    private static final class EnterTransform implements CodeTransform {
        private final int side;
        private final ClassDesc entryPoint;
        private boolean injected;

        private EnterTransform(int side, ClassDesc entryPoint) {
            this.side = side;
            this.entryPoint = entryPoint;
        }

        @Override
        public void accept(CodeBuilder builder, CodeElement element) {
            if (!injected) {
                injected = true;
                builder.loadConstant(side);
                // Resolve the entry point in its defining loader, including server bundlers.
                builder.loadConstant(entryPoint);
                builder.invokestatic(AGENT, "entered", ENTERED);
            }
            builder.with(element);
        }
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
                throw new IllegalArgumentException(
                        "the title-menu target returns " + returnKind
                                + ", and the bridge is written for an int or for nothing");
            }
            if (isWorldOpen(target)) {
                requireWorldOpenShape();
            }
            if (isWorldPackRepository(target) && !hasReferenceAt(0)) {
                // A primitive loaded with aload would fail JVM verification.
                throw new IllegalArgumentException(
                        "the pack-repository target's first parameter is not a reference, and the"
                                + " storage access is loaded from there");
            }
        }

        /** Rejects descriptor drift that would emit invalid reference or boolean loads. */
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

        /** Whether the target declares a reference at one parameter. */
        private boolean hasReferenceAt(int parameter) {
            return targetType.parameterCount() > parameter
                    && TypeKind.from(targetType.parameterList().get(parameter))
                            == TypeKind.REFERENCE;
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

        /** Counts local slots, including two-slot long and double parameters. */
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
                int slot = target.isStatic() ? 0 : 1;
                for (ClassDesc parameter : targetType.parameterList()) {
                    TypeKind kind = TypeKind.from(parameter);
                    builder.loadLocal(kind, slot);
                    slot += kind.slotSize();
                }
                builder.invokestatic(HOOK, target.hookMethod(), hookType);
                if (target.refuses()) {
                    // A true verdict skips the void target's body.
                    builder.ifThen(blk -> blk.return_());
                }
                // Consume the hook verdict before invoking adapters.
                if (isWorldOpen(target)) {
                    // Close the storage access and stem on refusal before persistent mutation.
                    builder.aload(slot(0));
                    builder.aload(slot(2));
                    builder.iload(slot(targetType.parameterCount() - 1));
                    builder.invokestatic(TRANSFORMER, "openWorldPersistence", WORLD_OPEN);
                    builder.ifThen(blk -> blk.return_());
                } else if (isWorldSave(target)) {
                    builder.aload(0);
                    builder.invokestatic(TRANSFORMER, "saveWorldPersistence", WORLD_SAVE);
                } else if (isWorldPackRepository(target)) {
                    builder.aload(slot(0));
                    builder.invokestatic(TRANSFORMER, "openWorldDatapack", WORLD_PACK_REPOSITORY);
                } else if (isCreateWorldScreen(target)) {
                    builder.aload(0);
                    builder.invokestatic(TRANSFORMER, "createWorldScreen", CREATE_WORLD_SCREEN);
                } else if (isBuiltInFreeze(target)) {
                    builder.invokestatic(TRANSFORMER, "registerContent", REGISTER_CONTENT);
                } else if (isClientTick(target)) {
                    builder.invokestatic(TRANSFORMER, "performOutward", PERFORM_OUTWARD);
                }
            }
            // Every return path must install the menu button exactly once.
            if (isTitleMenu(target)
                    && element instanceof ReturnInstruction instruction
                    && instruction.typeKind() == returnKind) {
                addMenuButton(builder);
            }
            builder.with(element);
        }

        /** Preserves the returned row for int builders; reads column arguments for void builders. */
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
