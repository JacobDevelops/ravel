package dev.ravel.jvm;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassModel;
import java.lang.classfile.MethodModel;
import java.lang.classfile.TypeKind;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.util.Arrays;

/** Byte-level checks use the selected inventory, not modern method names. */
public final class GenericMain {
    private GenericMain() { }

    static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static byte[] target(HookTarget target) {
        return ClassFile.of().build(ClassDesc.ofInternalName(target.internalClass()), builder -> {
            builder.withFlags(ClassFile.ACC_PUBLIC);
            // A distribution class also carries main, even when this fixture selects only a tick hook.
            if (Agent.sideOf(target.internalClass()) != Agent.UNNAMED) {
                builder.withMethodBody("main", MethodTypeDesc.ofDescriptor("([Ljava/lang/String;)V"),
                        ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, code -> code.return_());
            }
            builder.withMethodBody(target.member(), MethodTypeDesc.ofDescriptor(target.descriptor()),
                    ClassFile.ACC_PUBLIC | (target.isStatic() ? ClassFile.ACC_STATIC : 0), code -> {
                        if (target.member().equals("<init>")) {
                            code.aload(0).invokespecial(ClassDesc.of("java.lang.Object"), "<init>",
                                    MethodTypeDesc.ofDescriptor("()V"));
                        }
                        if (target.hookMethod().equals("legacyGlobalPaletteSize")) {
                            String field = LegacyNames.member("LEGACY_BLOCK", "STATE_REGISTRY");
                            String size = LegacyNames.member("LEGACY_STATE_IDS", "size");
                            code.getstatic(ClassDesc.of(LegacyNames.className("LEGACY_BLOCK")),
                                    field.substring(0, field.indexOf(':')),
                                    ClassDesc.ofDescriptor(field.substring(field.indexOf(':') + 1)));
                            code.invokevirtual(ClassDesc.of(LegacyNames.className("LEGACY_STATE_IDS")),
                                    size.substring(0, size.indexOf(':')),
                                    MethodTypeDesc.ofDescriptor(size.substring(size.indexOf(':') + 1)));
                            code.pop();
                        }
                        TypeKind kind = TypeKind.from(MethodTypeDesc.ofDescriptor(target.descriptor()).returnType());
                        switch (kind) {
                            case VOID -> code.return_();
                            case REFERENCE -> code.aconst_null().areturn();
                            case LONG -> code.lconst_0().lreturn();
                            case DOUBLE -> code.dconst_0().dreturn();
                            case FLOAT -> code.fconst_0().freturn();
                            default -> code.iconst_0().ireturn();
                        }
                    });
        });
    }

    private static long calls(MethodModel method, String owner, String member) {
        return method.code().orElseThrow().elementStream()
                .filter(element -> element instanceof InvokeInstruction invoke
                        && invoke.owner().asInternalName().equals(owner)
                        && invoke.name().equalsString(member)).count();
    }

    /** Seeing either entry class before startup must still diagnose its absent main. */
    private static void missingEntryBeforeStartup() {
        require(!Agent.clientStarted(), "generic fixture unexpectedly selected client startup");
        for (String entry : new String[] {GameNames.CLIENT_ENTRY_POINT, GameNames.SERVER_ENTRY_POINT}) {
            String owner = entry.replace('.', '/');
            byte[] original = ClassFile.of().build(ClassDesc.ofInternalName(owner),
                    builder -> builder.withFlags(ClassFile.ACC_PUBLIC));
            Transformer transformer = new Transformer(new HookTarget[0], true);
            byte[] rewritten = transformer.transform(null, GenericMain.class.getClassLoader(),
                    owner, null, null, original);
            require(Arrays.equals(original, rewritten), "missing entry changed original " + entry);
            require(transformer.transformationFailures().contains(entry + "=missing entry method"),
                    "unselected entry lost missing-main diagnostic " + entry);
        }
    }

    private static void adapterFixtureTests(HookTarget hook, MethodModel method) {
            boolean entry = java.util.Set.of("legacyBlockRegister", "legacyItemRegister", "legacyClientTick",
                    "legacyServerTick", "legacyPayloadClient", "legacyPayloadServer", "legacyResourceReload",
                    "legacyRecipeRegister", "legacyWorldOpen", "legacyWorldOpenIntegrated", "legacyWorldSave",
                    "legacyRendererFrame", "legacyRendererTerrain", "legacyRendererLayer",
                    "legacyRendererUpdate",
                    "legacyRendererUpload", "legacyRendererUnload", "legacyRendererClose")
                    .contains(hook.hookMethod());
            boolean exit = java.util.Set.of("legacyBootstrap", "legacyBlockRegister", "legacyItemRegister",
                    "legacyBlockStateFromMetadata", "legacyBlockMetadataFromState", "legacyItemMetadata",
                    "legacyResourceReload", "legacyAtlasReload", "legacyRecipeRegister", "legacyWorldOpen",
                    "legacyWorldOpenIntegrated", "legacyScreenOpen", "legacyTitleInit",
                    "legacyRendererCompile", "legacyRendererWorkerComplete",
                    "legacyRendererTaskCreate", "legacyBakedQuad", "legacyBakedAo",
                    "legacyBakedTint").contains(hook.hookMethod());
            require(calls(method, "dev/ravel/jvm/Hook", "legacyEnter") == (entry ? 1 : 0),
                    "wrong reviewed entry adapter " + hook);
            require(calls(method, "dev/ravel/jvm/Hook", "legacyExit") == (exit ? 1 : 0),
                    "wrong reviewed success adapter " + hook);
    }

    private static void rendererFixtureTests(HookTarget hook, MethodModel method) {
            if (hook.hookMethod().equals("legacyRendererTaskCreate")) {
                long receivers = method.code().orElseThrow().elementStream()
                        .filter(element -> element instanceof java.lang.classfile.instruction.LoadInstruction load
                                && load.typeKind() == TypeKind.REFERENCE && load.slot() == 0).count();
                require(receivers == 2, "constructor success failed to load initialized receiver");
            }
            if (java.util.Set.of("legacyRendererCompile", "legacyRendererWorkerComplete",
                    "legacyRendererTaskCreate").contains(hook.hookMethod())) {
                byte[] mixed = ClassFile.of().build(ClassDesc.ofInternalName(hook.internalClass()),
                        builder -> builder.withMethodBody(hook.member(),
                                MethodTypeDesc.ofDescriptor(hook.descriptor()), ClassFile.ACC_PUBLIC, code -> {
                                    if (hook.member().equals("<init>")) {
                                        code.aload(0).invokespecial(ClassDesc.of("java.lang.Object"), "<init>",
                                                MethodTypeDesc.ofDescriptor("()V"));
                                    }
                                    code.iconst_0().ifThen(branch -> branch.aconst_null().athrow());
                                    code.return_();
                                }));
                byte[] transformed = new Transformer(new HookTarget[] {hook}, true).transform(
                        null, GenericMain.class.getClassLoader(), hook.internalClass(), null, null, mixed);
                require(transformed != null, "mixed renderer seam was not transformed");
                MethodModel branching = ClassFile.of().parse(transformed).methods().getFirst();
                require(calls(branching, "dev/ravel/jvm/Hook", "legacyExit") == 1,
                        "mixed renderer seam lost normal-return observation");
                var elements = branching.code().orElseThrow().elementStream().toList();
                int thrown = -1;
                int exited = -1;
                for (int index = 0; index < elements.size(); index++) {
                    if (elements.get(index) instanceof java.lang.classfile.Instruction instruction
                            && instruction.opcode() == java.lang.classfile.Opcode.ATHROW) thrown = index;
                    if (elements.get(index) instanceof InvokeInstruction invoke
                            && invoke.owner().asInternalName().equals("dev/ravel/jvm/Hook")
                            && invoke.name().equalsString("legacyExit")) exited = index;
                }
                require(thrown >= 0 && exited > thrown,
                        "normal-return observation moved before exceptional branch");
            }
    }

    public static void main(String[] arguments) {
        require(Hook.LEGACY_ADAPTER, "not the legacy adapter");
        missingEntryBeforeStartup();
        java.util.Set<String> required = new java.util.HashSet<>(java.util.List.of(
                "legacyBootstrap", "legacyBlockRegister", "legacyItemRegister",
                "legacyBlockStateFromMetadata", "legacyBlockMetadataFromState", "legacyItemMetadata",
                "legacyClientTick", "legacyServerTick", "legacyIntegratedServerTick", "legacyServerWorldTick",
                "legacyPayloadClient", "legacyPayloadServer", "legacyResourceReload", "legacyAtlasReload",
                "legacyRecipeRegister", "legacySavedDataDirty", "legacySavedDataLoad", "legacySavedDataSet",
                "legacySavedDataSave", "legacyWorldSave", "legacyScreenOpen", "legacyScreenDraw",
                "legacyWorldOpen", "legacyWorldOpenIntegrated", "legacyCommandInstall",
                "legacyCommandExecute", "legacyItemFireDamage", "legacyTitleInit", "legacyGlobalPaletteSize",
                "legacyRendererFrame", "legacyRendererTerrain", "legacyRendererLayer",
                "legacyRendererUpdate", "legacyRendererCompile", "legacyRendererWorker",
                "legacyRendererWorkerComplete", "legacyRendererUpload", "legacyRendererUnload",
                "legacyRendererClose", "legacyRendererTaskCreate"));
        for (HookTarget hook : Hook.TARGETS) required.remove(hook.hookMethod());
        require(required.isEmpty(), "missing original/supplemental legacy hooks: " + required);
        for (HookTarget hook : Hook.TARGETS) {
            Transformer transformer = new Transformer(new HookTarget[] {hook}, true);
            byte[] original = target(hook);
            byte[] rewritten = transformer.transform(null, GenericMain.class.getClassLoader(),
                    hook.internalClass(), null, null, original);
            require(rewritten != null && !Arrays.equals(original, rewritten), "unchanged " + hook);
            ClassModel model = ClassFile.of().parse(rewritten);
            if (Agent.sideOf(hook.internalClass()) != Agent.UNNAMED) {
                MethodModel entry = model.methods().stream().filter(candidate ->
                        candidate.methodName().equalsString("main")).findFirst().orElseThrow();
                require(calls(entry, "dev/ravel/jvm/Agent", "entered") == 1,
                        "missing distribution entry prologue " + hook);
            }
            MethodModel method = model.methods().stream().filter(candidate ->
                    candidate.methodName().equalsString(hook.member())).findFirst().orElseThrow();
            require(calls(method, "dev/ravel/jvm/Hook", hook.hookMethod()) == 1, "missing native prologue " + hook);
            require(calls(method, "dev/ravel/jvm/Hook", "legacyObserve") == 1, "missing observation " + hook);
            adapterFixtureTests(hook, method);
            rendererFixtureTests(hook, method);
            for (String wrapper : java.util.List.of("Integer", "Long", "Float", "Double", "Boolean")) {
                require(calls(method, "java/lang/" + wrapper, "valueOf") == 0, "boxing allocation " + hook);
            }
            require(transformer.isInstalled(hook.hookMethod()) && transformer.rewrittenMethods() == 1
                    && transformer.transformedClasses() == 1, "unpublished rewrite " + hook);
            require(transformer.transformationFailures().isEmpty(), "failed " + hook);
        }
        HookTarget good = new HookTarget("fixture/Partial", "run", "()V", true, "probe", "()V");
        HookTarget bad = new HookTarget("fixture/Partial", "run", "()V", true, "probe", "not-a-descriptor");
        Transformer partial = new Transformer(new HookTarget[] {good, bad}, true);
        byte[] original = target(good);
        byte[] snapshot = original.clone();
        require(partial.transform(null, GenericMain.class.getClassLoader(), good.internalClass(), null, null, original) == null,
                "failed transform published partially rewritten bytes");
        require(Arrays.equals(original, snapshot) && !partial.isInstalled("probe")
                && partial.rewrittenMethods() == 0 && partial.transformedClasses() == 0
                && partial.transformationFailures().contains("fixture.Partial"), "failure lost original/evidence");
        require(partial.transform(null, null, null, null, null, original) == null, "unnamed transform changed bytes");
        require(partial.transform(null, null, "fixture/Unknown", null, null, original) == null, "unknown target changed bytes");
        System.out.println("legacy transformation checked=" + Hook.TARGETS.length + " failed-transform=preserved");
    }
}
