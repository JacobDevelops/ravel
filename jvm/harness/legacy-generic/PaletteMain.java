package dev.ravel.jvm;

import java.util.Arrays;

/** Verifies exact field/call provenance and executes the rewritten palette bit calculation. */
public final class PaletteMain {
    private static final String OWNER = "dev/ravel/jvm/PaletteFixture";
    private PaletteMain() {}
    private static final class Definer extends ClassLoader {
        Definer() { super(PaletteMain.class.getClassLoader()); }
        Class<?> define(byte[] bytes) { return defineClass(OWNER.replace('/', '.'), bytes, 0, bytes.length); }
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    private static HookTarget target(String method) {
        return new HookTarget(OWNER, method, "(I)V", false, "legacyGlobalPaletteSize", "(I)V");
    }
    public static void main(String[] ignored) throws Exception {
        byte[] original;
        try (var input = PaletteMain.class.getClassLoader().getResourceAsStream(OWNER + ".class")) {
            check(input != null, "missing compiled palette fixture");
            original = input.readAllBytes();
        }
        Transformer transformer = new Transformer(new HookTarget[] {target("resize")}, true);
        Definer loader = new Definer();
        byte[] rewritten = transformer.transform(null, loader, OWNER, null, null, original);
        check(rewritten != null && !Arrays.equals(original, rewritten)
                && transformer.isInstalled("legacyGlobalPaletteSize") && transformer.rewrittenMethods() == 1,
                "exact global-palette field/size call not transformed");
        Class<?> type = loader.define(rewritten);
        Object instance = type.getConstructor().newInstance();
        int calls = Hook.calls;
        int paletteCalls = Hook.paletteCalls;
        type.getMethod("resize", int.class).invoke(instance, 4);
        check(type.getField("bits").getInt(instance) == 13, "global palette did not expand to fit fixture ID range");
        check(Hook.calls == calls + 1 && Hook.paletteCalls == paletteCalls + 1 && Hook.paletteInput == 13,
                "palette native entry/size replacement did not execute exactly once");
        for (String method : new String[] {"absent", "twice", "wrong"}) {
            Transformer rejected = new Transformer(new HookTarget[] {target(method)}, true);
            byte[] snapshot = original.clone();
            byte[] result = rejected.transform(null, loader, OWNER, null, null, original);
            check(result == null && Arrays.equals(original, snapshot)
                    && !rejected.isInstalled("legacyGlobalPaletteSize")
                    && rejected.rewrittenMethods() == 0 && rejected.transformedClasses() == 0
                    && rejected.transformationFailures().contains(OWNER.replace('/', '.')),
                    "ambiguous/missing/wrong-field palette call did not fail closed: " + method);
        }
        System.out.println("legacy global-palette exact-call=executed expanded-bits=13 invalid-provenance=refused");
    }
}
