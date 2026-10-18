package dev.ravel.jvm;

/** Compiled only against the isolated observation Hook. */
public final class InitializationMain {
    private InitializationMain() { }
    public static void run(String[] arguments) {
        int side = arguments[0].equals("client") ? 0 : 1;
        if (Hook.initializations != 1 || Hook.side != side || Hook.calls != 1
                || fixture.InitTarget.bodies != 1 || !Agent.isInstalled("probe")
                || Agent.transformedClasses() != 1 || Agent.rewrittenMethods() != 1) {
            throw new AssertionError("initialization-loaded target escaped transformation or side initialization");
        }
        missingEntryMethods(side);
        Agent.entered(1 - side, InitializationMain.class);
        if (Hook.initializations != 1 || Hook.side != side || Hook.calls != 1) {
            throw new AssertionError("later entrant reinitialized the first side");
        }
        System.out.println("legacy initialization side=" + arguments[0] + " target=rewritten-before-body");
    }

    /** A started client may load the integrated server, but its own entry remains mandatory. */
    private static void missingEntryMethods(int selectedSide) {
        for (String entry : new String[] {GameNames.CLIENT_ENTRY_POINT, GameNames.SERVER_ENTRY_POINT}) {
            String owner = entry.replace('.', '/');
            HookTarget hook = new HookTarget(owner, "run", "()V", true, "probe", "()V");
            byte[] original = java.lang.classfile.ClassFile.of().build(
                    java.lang.constant.ClassDesc.ofInternalName(owner), builder -> {
                        builder.withFlags(java.lang.classfile.ClassFile.ACC_PUBLIC);
                        builder.withMethodBody("run", java.lang.constant.MethodTypeDesc.ofDescriptor("()V"),
                                java.lang.classfile.ClassFile.ACC_PUBLIC | java.lang.classfile.ClassFile.ACC_STATIC,
                                code -> code.return_());
                    });
            Transformer transformer = new Transformer(new HookTarget[] {hook}, true);
            byte[] rewritten = transformer.transform(null, InitializationMain.class.getClassLoader(),
                    owner, null, null, original);
            boolean integrated = selectedSide == Agent.sideOf(
                    GameNames.CLIENT_ENTRY_POINT.replace('.', '/')) && entry.equals(GameNames.SERVER_ENTRY_POINT);
            if (rewritten == null || !transformer.isInstalled("probe")
                    || transformer.rewrittenMethods() != 1 || transformer.transformedClasses() != 1) {
                throw new AssertionError("entry without main lost its ordinary hook: " + entry);
            }
            if (integrated ? !transformer.transformationFailures().isEmpty()
                    : !transformer.transformationFailures().contains(entry + "=missing entry method")) {
                throw new AssertionError("missing main diagnostic disagrees with selected side: " + entry);
            }
        }
    }
}
