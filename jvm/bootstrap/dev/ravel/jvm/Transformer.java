package dev.ravel.jvm;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.classfile.Instruction;
import java.lang.classfile.MethodTransform;
import java.lang.classfile.Opcode;
import java.lang.classfile.TypeKind;
import java.lang.classfile.instruction.FieldInstruction;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.classfile.instruction.ReturnInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.AccessFlag;
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
 * Native hooks retain declared arguments; legacy adapters also receive receivers and successful results.
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
    private static final ClassDesc INTEGER = ClassDesc.of("java.lang.Integer");
    private static final MethodTypeDesc LEGACY_ENTER = MethodTypeDesc.ofDescriptor(
            "(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;I)V");
    private static final MethodTypeDesc LEGACY_EXIT = MethodTypeDesc.ofDescriptor(
            "(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;ILjava/lang/Object;I)V");
    private static final MethodTypeDesc LEGACY_COMMAND = MethodTypeDesc.ofDescriptor(
            "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/Integer;");
    private static final MethodTypeDesc LEGACY_REFUSE = MethodTypeDesc.ofDescriptor(
            "(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;)Z");
    private static final MethodTypeDesc LEGACY_PALETTE_SIZE =
            MethodTypeDesc.ofDescriptor("(I)I");

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
        try {
            return (int) ModernAdapters.TITLE.invokeExact(titleScreen, bottom, spacing);
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Uses the version's row offset for title-menu builders that return no row. */
    public static void addTitleMenuColumn(Object titleScreen, int top, int spacing) {
        addTitleMenu(titleScreen, top + MenuLayout.RAVEL_ROW - spacing, spacing);
    }

    /** Exposes the pack writer to transformed game code. */
    public static void openWorldDatapack(Object access) {
        try {
            ModernAdapters.PACK.invokeExact(access);
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Exposes create-world pack staging to transformed game code. */
    public static void createWorldScreen(Object screen) {
        try {
            ModernAdapters.STAGE.invokeExact(screen);
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Returns whether the world-load body must be skipped. */
    public static boolean openWorldPersistence(Object access, Object stem, boolean freshWorld) {
        try {
            return (boolean) ModernAdapters.OPEN.invokeExact(access, stem, freshWorld);
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Gives the rewritten save method a public path to the persistence adapter. */
    public static void saveWorldPersistence(Object server) {
        try {
            ModernAdapters.SAVE.invokeExact(server);
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Registers admitted content after the hook, before built-in registries freeze. */
    public static void registerContent() {
        try {
            ModernAdapters.REGISTER.invokeExact();
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    /** Drains outward actions after the client-tick hook. */
    public static void performOutward() {
        try {
            ModernAdapters.OUTWARD.invokeExact();
        } catch (Throwable failure) {
            throw adapterFailure(failure);
        }
    }

    private static RuntimeException adapterFailure(Throwable failure) {
        if (failure instanceof RuntimeException runtime) {
            return runtime;
        }
        if (failure instanceof Error error) {
            throw error;
        }
        return new IllegalStateException("Minecraft adapter failed", failure);
    }

    /** Links modern adapters only on a modern seam, with no per-tick reflection or boxing. */
    private static final class ModernAdapters {
        private static final MethodHandle TITLE = link("Screens", "addTitleMenu",
                int.class, Object.class, int.class, int.class);
        private static final MethodHandle PACK = link("Datapack", "write", void.class, Object.class);
        private static final MethodHandle STAGE = link("Datapack", "stage", void.class, Object.class);
        private static final MethodHandle OPEN = link("Persistence", "open",
                boolean.class, Object.class, Object.class, boolean.class);
        private static final MethodHandle SAVE = link("Persistence", "save", void.class, Object.class);
        private static final MethodHandle REGISTER = link("Content", "register", void.class);
        private static final MethodHandle OUTWARD = link("Outward", "perform", void.class);

        private static MethodHandle link(
                String owner, String member, Class<?> result, Class<?>... parameters) {
            try {
                Class<?> type = Class.forName(
                        "dev.ravel.jvm." + owner, false, Transformer.class.getClassLoader());
                return MethodHandles.lookup().findStatic(
                        type, member, MethodType.methodType(result, parameters));
            } catch (ReflectiveOperationException failure) {
                throw new IllegalStateException("Cannot link adapter: " + owner + "." + member, failure);
            }
        }
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
                if (prepend.dormantBaked) continue;
                rewritten =
                        context.transformClass(
                                context.parse(rewritten),
                                ClassTransform.transformingMethods(
                                        model ->
                                                model.methodName().equalsString(target.member())
                                                        && model.methodType()
                                                                .equalsString(target.descriptor()),
                                        MethodTransform.transformingCode(prepend)));
                if (target.hookMethod().equals("legacyRendererUpload")) {
                    rewritten = context.transformClass(context.parse(rewritten),
                            ClassTransform.transformingMethods(model -> model.methodType().equalsString("()V")
                                            && !model.flags().has(AccessFlag.STATIC)
                                            && !model.methodName().equalsString("<init>"),
                                    MethodTransform.transformingCode((builder, element) -> {
                                        if (element instanceof ReturnInstruction) {
                                            builder.aload(0);
                                            builder.invokestatic(ClassDesc.of("dev.ravel.jvm.LegacyRenderer"),
                                                    "vanillaBufferDeleted",
                                                    MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V"));
                                        }
                                        builder.with(element);
                                    })));
                }
                prepend.requireComplete();
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
        return matched.isEmpty() ? entered : rewritten;
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
                // The client jar also contains the integrated server class, without a main.
                if (!Agent.clientStarted() || side == Agent.sideOf(
                        GameNames.CLIENT_ENTRY_POINT.replace('.', '/'))) {
                    failedByClass.put(internalName.replace('/', '.'), MISSING_ENTRY_METHOD);
                }
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
        private int legacyFirst;
        private int legacySecond;
        private int legacyNumber;
        private int paletteCalls;
        private boolean paletteField;
        private int legacyReceiver;
        private final boolean dormantBaked;

        private PrependTransform(HookTarget target) {
            this.target = target;
            targetType = MethodTypeDesc.ofDescriptor(target.descriptor());
            hookType = MethodTypeDesc.ofDescriptor(target.hookDescriptor());
            returnKind = TypeKind.from(targetType.returnType()).asLoadable();
            dormantBaked = target.hookMethod().startsWith("legacyBaked")
                    && System.getProperty("ravel.renderer", "vanilla").equals("vanilla");
            if (isTitleMenu(target)
                    && TypeKind.from(targetType.returnType()) != TypeKind.INT
                    && returnKind != TypeKind.VOID) {
                throw new IllegalArgumentException(
                        "the title-menu target returns " + returnKind
                                + ", and the bridge is written for an int or for nothing");
            }
            if (isWorldOpen(target)) {
                requireWorldOpenShape();
            }
            requireLegacyShape();
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

        /** A reviewed palette target must contain exactly one global registry size seam. */
        private void requireComplete() {
            if (isLegacyPalette() && (!injected || paletteCalls != 1)) {
                throw new IllegalArgumentException(
                        "the legacy global palette requires exactly one state registry size call");
            }
        }

        private boolean isLegacyPalette() {
            return switch (target.hookMethod()) {
                case "legacyGlobalPaletteSize" -> true;
                default -> false;
            };
        }

        /** Widens the size result locally, without changing the registry's size semantics. */
        private void adaptPalette(CodeBuilder builder, CodeElement element) {
            if (!isLegacyPalette()) {
                return;
            }
            if (element instanceof InvokeInstruction call
                    && Hook.legacyPaletteSizeCall(
                            call.owner().name().stringValue(),
                            call.name().stringValue(), call.type().stringValue())) {
                if (!paletteField || call.opcode() != Opcode.INVOKEVIRTUAL) {
                    throw new IllegalArgumentException(
                            "the legacy palette size call must read the global state registry");
                }
                paletteCalls++;
                builder.invokestatic(HOOK, "legacyPaletteSize", LEGACY_PALETTE_SIZE);
            }
            if (element instanceof Instruction) {
                paletteField = element instanceof FieldInstruction field
                        && field.opcode() == Opcode.GETSTATIC
                        && Hook.legacyPaletteField(field.owner().name().stringValue(),
                                field.name().stringValue(), field.type().stringValue());
            }
        }

        /** Counts local slots, including two-slot long and double parameters. */
        private int slot(int parameter) {
            int at = target.isStatic() ? 0 : 1;
            for (int index = 0; index < parameter; index++) {
                at += TypeKind.from(targetType.parameterList().get(index)).slotSize();
            }
            return at;
        }

        private void freezeCompositeUpdate(CodeBuilder builder) {
            if (target.hookMethod().equals("legacyRendererUpdate")
                    && !System.getProperty("ravel.renderer", "vanilla").equals("vanilla")) {
                builder.invokestatic(ClassDesc.of("dev.ravel.jvm.LegacyRenderer"),
                        "skipCompositeUpdate", MethodTypeDesc.ofDescriptor("()Z"));
                builder.ifThen(block -> block.return_());
            }
        }

        @Override
        public void accept(CodeBuilder builder, CodeElement element) {
            if (!injected) {
                injected = true;
                freezeCompositeUpdate(builder);
                int slot = target.isStatic() ? 0 : 1;
                for (ClassDesc parameter : targetType.parameterList()) {
                    TypeKind kind = TypeKind.from(parameter);
                    builder.loadLocal(kind, slot);
                    slot += kind.slotSize();
                }
                builder.invokestatic(HOOK, target.hookMethod(), hookType);
                if (target.hookMethod().startsWith("legacy")) {
                    builder.loadConstant(target.hookMethod());
                    builder.invokestatic(HOOK, "legacyObserve",
                            MethodTypeDesc.ofDescriptor("(Ljava/lang/String;)V"));
                }
                if ((legacyEntry() || legacyExit()) && !target.hookMethod().equals("legacyRendererTaskCreate")) {
                    enterLegacy(builder);
                }
                bakedEntry(builder);
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
            if (target.hookMethod().equals("legacyBakedPosition")
                    && element instanceof ReturnInstruction) {
                builder.aload(0);
                for (int i = 0; i < 3; i++) builder.dload(slot(i));
                builder.invokestatic(HOOK, "legacyCapturePosition",
                        MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;DDD)V"));
            }
            if (target.hookMethod().equals("legacyRendererUpload")
                    && element instanceof ReturnInstruction) {
                builder.aload(0);
                builder.aload(slot(0));
                builder.invokestatic(HOOK, "legacyCaptureUpload",
                        MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;Ljava/lang/Object;)V"));
            }
            if (legacyExit()
                    && element instanceof ReturnInstruction instruction
                    && instruction.typeKind() == returnKind) {
                // A constructor receiver is usable only after vanilla initialization succeeded.
                if (target.hookMethod().equals("legacyRendererTaskCreate")) enterLegacy(builder);
                exitLegacy(builder);
            }
            builder.with(element);
            adaptPalette(builder, element);
        }

        /** Only reviewed adapter entry seams receive semantic callbacks. */
        private boolean legacyEntry() {
            return switch (target.hookMethod()) {
                case "legacyBlockRegister", "legacyItemRegister", "legacyClientTick",
                        "legacyServerTick", "legacyPayloadClient", "legacyPayloadServer",
                        "legacyResourceReload", "legacyAtlasReload", "legacyRecipeRegister", "legacyWorldOpen",
                        "legacyWorldOpenIntegrated", "legacyWorldSave",
                        "legacyCommandExecute", "legacyItemFireDamage",
                        "legacyRendererFrame", "legacyRendererTerrain", "legacyRendererLayer",
                        "legacyRendererUpdate",
                        "legacyRendererUpload", "legacyRendererUnload", "legacyRendererClose" -> true;
                default -> false;
            };
        }

        private boolean legacyExit() {
            return switch (target.hookMethod()) {
                case "legacyBootstrap", "legacyBlockRegister", "legacyItemRegister",
                        "legacyBlockStateFromMetadata", "legacyBlockMetadataFromState",
                        "legacyItemMetadata", "legacyResourceReload", "legacyAtlasReload",
                        "legacyRecipeRegister", "legacyWorldOpen", "legacyWorldOpenIntegrated",
                        "legacyScreenOpen", "legacyTitleInit", "legacyRendererCompile",
                        "legacyRendererWorkerComplete", "legacyRendererTaskCreate",
                        "legacyBakedQuad", "legacyBakedAo", "legacyBakedTint", "legacyRendererLayer" -> true;
                default -> false;
            };
        }

        private void bakedEntry(CodeBuilder builder) {
            String method = target.hookMethod();
            String descriptor;
            switch (method) {
                case "legacyRendererCompile" -> {
                    builder.aload(slot(3));
                    for (int i = 0; i < 3; i++) builder.fload(slot(i));
                    descriptor = "(Ljava/lang/Object;FFF)V";
                    method = "legacyCaptureBegin";
                }
                case "legacyBakedSort" -> {
                    builder.aload(0);
                    for (int i = 0; i < 3; i++) builder.fload(slot(i));
                    descriptor = "(Ljava/lang/Object;FFF)V";
                    method = "legacyCaptureSort";
                }
                case "legacyBakedModel" -> {
                    builder.aload(slot(2));
                    builder.aload(slot(4));
                    descriptor = "(Ljava/lang/Object;Ljava/lang/Object;)V";
                    method = "legacyCaptureModel";
                }
                case "legacyBakedVertices" -> {
                    builder.aload(0);
                    builder.aload(slot(0));
                    descriptor = "(Ljava/lang/Object;[I)V";
                    method = "legacyCaptureVertices";
                }
                case "legacyBakedLight" -> {
                    builder.aload(0);
                    for (int i = 0; i < 4; i++) builder.iload(slot(i));
                    descriptor = "(Ljava/lang/Object;IIII)V";
                    method = "legacyCaptureLight";
                }
                case "legacyBakedColor" -> {
                    builder.aload(0);
                    for (int i = 0; i < 3; i++) builder.fload(slot(i));
                    builder.iload(slot(3));
                    descriptor = "(Ljava/lang/Object;FFFI)V";
                    method = "legacyCaptureColor";
                }
                default -> { return; }
            }
            builder.invokestatic(HOOK, method, MethodTypeDesc.ofDescriptor(descriptor));
        }

        /** Retains only semantically consumed arguments, without arrays or boxing. */
        private void enterLegacy(CodeBuilder builder) {
            legacyReceiver = builder.allocateLocal(TypeKind.REFERENCE);
            legacyFirst = builder.allocateLocal(TypeKind.REFERENCE);
            legacySecond = builder.allocateLocal(TypeKind.REFERENCE);
            legacyNumber = builder.allocateLocal(TypeKind.INT);
            if (target.hookMethod().equals("legacyRendererTaskCreate")) builder.aload(0);
            else receiver(builder);
            builder.astore(legacyReceiver);
            captureReference(builder, legacyFirst, firstParameter());
            captureReference(builder, legacySecond, secondParameter());
            switch (target.hookMethod()) {
                case "legacyBlockStateFromMetadata", "legacyBlockRegister", "legacyItemRegister" ->
                        builder.iload(slot(0));
                default -> builder.iconst_0();
            }
            builder.istore(legacyNumber);
            switch (target.hookMethod()) {
                case "legacyCommandExecute" -> commandLegacy(builder);
                case "legacyItemFireDamage" -> {
                    builder.loadConstant(target.hookMethod());
                    builder.aload(legacyReceiver);
                    builder.aload(legacyFirst);
                    builder.invokestatic(HOOK, "legacyRefuse", LEGACY_REFUSE);
                    builder.ifThen(refused -> {
                        refused.iconst_0();
                        refused.ireturn();
                    });
                }
                default -> {
                    if (legacyEntry()) {
                        legacyContext(builder);
                        builder.invokestatic(HOOK, "legacyEnter", LEGACY_ENTER);
                    }
                }
            }
        }

        private int firstParameter() {
            return switch (target.hookMethod()) {
                case "legacyBlockRegister", "legacyItemRegister" -> 1;
                case "legacyPayloadClient", "legacyPayloadServer", "legacyRecipeRegister",
                        "legacyWorldOpen", "legacyWorldOpenIntegrated", "legacyScreenOpen",
                        "legacyBlockMetadataFromState", "legacyCommandExecute",
                        "legacyItemFireDamage", "legacyRendererLayer" -> 0;
                case "legacyRendererCompile" -> 3;
                default -> -1;
            };
        }

        private int secondParameter() {
            return switch (target.hookMethod()) {
                case "legacyBlockRegister", "legacyItemRegister" -> 2;
                case "legacyRecipeRegister", "legacyCommandExecute" -> 1;
                default -> -1;
            };
        }

        private void captureReference(CodeBuilder builder, int destination, int parameter) {
            if (parameter < 0) {
                builder.aconst_null();
            } else {
                builder.aload(slot(parameter));
            }
            builder.astore(destination);
        }

        private void receiver(CodeBuilder builder) {
            if (target.isStatic() || target.member().equals("<init>")) {
                builder.aconst_null();
            } else {
                builder.aload(0);
            }
        }

        private void legacyContext(CodeBuilder builder) {
            builder.loadConstant(target.hookMethod());
            builder.aload(legacyReceiver);
            builder.aload(legacyFirst);
            builder.aload(legacySecond);
            builder.iload(legacyNumber);
        }

        /** Observes an actual normal return and restores its untouched primitive/reference value. */
        private void exitLegacy(CodeBuilder builder) {
            int result = -1;
            if (returnKind != TypeKind.VOID) {
                result = builder.allocateLocal(returnKind);
                builder.storeLocal(returnKind, result);
            }
            legacyContext(builder);
            if (returnKind == TypeKind.REFERENCE) {
                builder.aload(result);
            } else {
                builder.aconst_null();
            }
            if (returnKind == TypeKind.INT) {
                builder.iload(result);
            } else {
                builder.iconst_0();
            }
            builder.invokestatic(HOOK, "legacyExit", LEGACY_EXIT);
            if (returnKind != TypeKind.VOID) {
                builder.loadLocal(returnKind, result);
            }
        }

        private void commandLegacy(CodeBuilder builder) {
            int decision = builder.allocateLocal(TypeKind.REFERENCE);
            builder.aload(legacyReceiver);
            builder.aload(legacyFirst);
            builder.aload(legacySecond);
            builder.checkcast(ClassDesc.of("java.lang.String"));
            builder.invokestatic(HOOK, "legacyCommand", LEGACY_COMMAND);
            builder.astore(decision);
            builder.aload(decision);
            builder.ifThen(java.lang.classfile.Opcode.IFNONNULL, accepted -> {
                accepted.aload(decision);
                accepted.invokevirtual(INTEGER, "intValue", MethodTypeDesc.ofDescriptor("()I"));
                accepted.ireturn();
            });
        }

        /** Rejects descriptor drift before emitting typed local loads or early returns. */
        private void requireLegacyShape() {
            requireLegacyCaptureShape();
            switch (target.hookMethod()) {
                case "legacyCommandExecute" -> {
                    if (target.isStatic() || targetType.parameterCount() != 2
                            || !hasReferenceAt(0)
                            || !targetType.parameterType(1).descriptorString().equals("Ljava/lang/String;")
                            || !targetType.returnType().descriptorString().equals("I")) {
                        throw new IllegalArgumentException(
                                "Legacy command execution requires (sender,String)int");
                    }
                }
                case "legacyItemFireDamage" -> {
                    if (target.isStatic() || targetType.parameterCount() != 2
                            || !hasReferenceAt(0)
                            || !targetType.parameterType(1).descriptorString().equals("F")
                            || !targetType.returnType().descriptorString().equals("Z")) {
                        throw new IllegalArgumentException(
                                "Legacy fire damage requires (source,float)boolean");
                    }
                }
                case "legacyWorldOpen", "legacyWorldOpenIntegrated" -> {
                    if (target.isStatic() || targetType.parameterCount() != 5
                            || !targetType.parameterType(0).descriptorString().equals("Ljava/lang/String;")
                            || !targetType.parameterType(1).descriptorString().equals("Ljava/lang/String;")
                            || !targetType.parameterType(2).descriptorString().equals("J")
                            || !hasReferenceAt(3)
                            || !targetType.parameterType(4).descriptorString().equals("Ljava/lang/String;")
                            || returnKind != TypeKind.VOID) {
                        throw new IllegalArgumentException(
                                "Legacy world opening requires (String,String,long,type,String)void");
                    }
                }
                default -> { }
            }
        }

        /** Mirrors each typed adapter's local loads, receiver use and result interpretation. */
        private void requireLegacyCaptureShape() {
            String expected = switch (target.hookMethod()) {
                case "legacyBootstrap" -> "static:()V";
                case "legacyBlockRegister", "legacyItemRegister" -> "static:(ILL)V";
                case "legacyBlockStateFromMetadata" -> "static:(I)L";
                case "legacyBlockMetadataFromState" -> "static:(L)I";
                case "legacyRecipeRegister" -> "static:(LL)V";
                case "legacyItemMetadata" -> "instance:()I";
                case "legacyClientTick", "legacyServerTick", "legacyResourceReload",
                        "legacyTitleInit" -> "instance:()V";
                case "legacyPayloadClient", "legacyPayloadServer", "legacyScreenOpen",
                        "legacyAtlasReload" -> "instance:(L)V";
                case "legacyGlobalPaletteSize" -> "instance:(I)V";
                case "legacyWorldSave" -> "instance:(Z)V";
                case "legacyWorldOpen", "legacyWorldOpenIntegrated" -> "instance:(LLJLL)V";
                case "legacyCommandExecute" -> "instance:(LL)I";
                case "legacyItemFireDamage" -> "instance:(LF)Z";
                case "legacyRendererFrame", "legacyRendererClose" -> "instance:()V";
                case "legacyRendererTerrain" -> "instance:(LDLIZ)V";
                case "legacyRendererLayer" -> "instance:(LDIL)I";
                case "legacyRendererUpdate" -> "instance:(J)V";
                case "legacyRendererCompile" -> "instance:(FFFL)V";
                case "legacyRendererWorker", "legacyRendererWorkerComplete",
                        "legacyRendererUpload" -> "instance:(L)V";
                case "legacyRendererUnload" -> "instance:(LL)V";
                case "legacyRendererTaskCreate" -> "instance:(LLD)V";
                case "legacyBakedModel" -> "instance:(LLLLLZJ)Z";
                case "legacyBakedQuad" -> "instance:()L";
                case "legacyBakedAo" -> "instance:(LLLLLL)V";
                case "legacyBakedTint" -> "instance:(LLLI)I";
                case "legacyBakedVertices" -> "instance:(L)V";
                case "legacyBakedLight" -> "instance:(IIII)V";
                case "legacyBakedColor" -> "instance:(FFFI)V";
                case "legacyBakedPosition" -> "instance:(DDD)V";
                case "legacyBakedSort" -> "instance:(FFF)V";
                default -> null;
            };
            if (expected == null) {
                return;
            }
            StringBuilder actual = new StringBuilder(target.isStatic() ? "static:(" : "instance:(");
            for (ClassDesc parameter : targetType.parameterList()) {
                actual.append(captureKind(parameter));
            }
            actual.append(')').append(captureKind(targetType.returnType()));
            if (!expected.contentEquals(actual)) {
                throw new IllegalArgumentException(target.hookMethod()
                        + " adapter capture requires " + expected + ", found " + actual);
            }
            requireBakedArrayShape();
        }

        private void requireBakedArrayShape() {
            String method = target.hookMethod();
            boolean valid = switch (method) {
                case "legacyBakedQuad" -> targetType.returnType().descriptorString().equals("[I");
                case "legacyBakedVertices" -> targetType.parameterType(0).descriptorString().equals("[I");
                case "legacyBakedAo" -> targetType.parameterType(4).descriptorString().equals("[F");
                default -> true;
            };
            if (!valid) throw new IllegalArgumentException(method + " requires its reviewed primitive array");
        }

        private static String captureKind(ClassDesc type) {
            return TypeKind.from(type) == TypeKind.REFERENCE ? "L" : type.descriptorString();
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
