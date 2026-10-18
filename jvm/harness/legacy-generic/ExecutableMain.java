package dev.ravel.jvm;

import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.reflect.InvocationTargetException;
import java.util.Arrays;

/** Every assertion below executes JVM-verified rewritten ordinary Java methods. */
public final class ExecutableMain {
    private static final String OWNER = "fixture/ExecutableTarget";
    private static final String OPEN = "(Ljava/lang/String;Ljava/lang/String;JLjava/lang/Object;Ljava/lang/String;)V";
    private ExecutableMain() {}

    private static HookTarget target(String member, String descriptor, String hook) {
        return target(member, descriptor, hook, false);
    }

    private static HookTarget target(String member, String descriptor, String hook, boolean isStatic) {
        // Initialize ConstantDescs before parsing primitive descriptors (JDK 27 initialization order).
        ClassDesc returnType = ConstantDescs.CD_void;
        String nativeDescriptor = MethodTypeDesc.ofDescriptor(descriptor)
                .changeReturnType(returnType).descriptorString();
        return new HookTarget(OWNER, member, descriptor, isStatic, hook, nativeDescriptor);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class DefiningLoader extends ClassLoader {
        DefiningLoader() { super(ExecutableMain.class.getClassLoader()); }
        Class<?> define(byte[] bytes) { return defineClass(OWNER.replace('/', '.'), bytes, 0, bytes.length); }
    }

    public static void main(String[] ignored) throws Exception {
        HookTarget[] targets = {
            target("<init>", "(Ljava/lang/Object;)V", "legacyCommandInstall"),
            target("open", OPEN, "legacyWorldOpen"),
            target("blockRegister", "(ILjava/lang/Object;Ljava/lang/Object;)V", "legacyBlockRegister", true),
            target("itemRegister", "(ILjava/lang/Object;Ljava/lang/Object;)V", "legacyItemRegister", true),
            target("recipe", "(Ljava/lang/Object;Ljava/lang/Object;)V", "legacyRecipeRegister", true),
            target("staticState", "(I)Ljava/lang/Object;", "legacyBlockStateFromMetadata", true),
            target("staticMetadata", "(Ljava/lang/Object;)I", "legacyBlockMetadataFromState", true),
            target("itemMetadata", "()I", "legacyItemMetadata"),
            target("command", "(Ljava/lang/Object;Ljava/lang/String;)I", "legacyCommandExecute"),
            target("fire", "(Ljava/lang/Object;F)Z", "legacyItemFireDamage"),
            target("throwing", OPEN, "legacyWorldOpen")
        };
        byte[] original;
        try (var input = ExecutableMain.class.getClassLoader().getResourceAsStream(OWNER + ".class")) {
            check(input != null, "missing compiled executable fixture");
            original = input.readAllBytes();
        }
        Transformer transformer = new Transformer(targets, true);
        DefiningLoader loader = new DefiningLoader();
        byte[] rewritten = transformer.transform(null, loader, OWNER, null, null, original);
        check(rewritten != null && !Arrays.equals(original, rewritten), "executable fixture not rewritten");
        check(transformer.rewrittenMethods() == targets.length && transformer.transformationFailures().isEmpty(),
                "executable fixture partially transformed");
        Class<?> type = loader.define(rewritten);
        Object marker = new Object();
        Hook.resetObservations();
        int observed = Hook.observations;
        Object receiver = type.getConstructor(Object.class).newInstance(marker);
        check(type.getField("initialized").get(receiver) == marker && bodies(type) == 1,
                "rewritten constructor did not initialize this/body");
        check(Hook.calls == 1 && Hook.observations == observed + 1,
                "constructor native/observation prologue did not execute once");
        check(Hook.entries.isEmpty() && Hook.exits.isEmpty(), "observation-only constructor invoked adapters");

        Object generator = new Object();
        long seed = 0x123456789abcdef0L;
        Class<?>[] openParameters = {String.class, String.class, long.class, Object.class, String.class};
        Hook.resetObservations();
        type.getMethod("open", openParameters).invoke(receiver, "save-name", "display-name", seed, generator, "settings");
        check(Hook.nativeSeed == seed && Hook.nativeGenerator == generator && Hook.nativeSettings.equals("settings"),
                "native prologue corrupted wide parameter slots or following references");
        check(type.getField("seed").getLong(receiver) == seed
                && type.getField("generator").get(receiver) == generator
                && type.getField("settings").get(receiver).equals("settings"), "original wide-argument body changed");
        check(Hook.entries.size() == 1 && Hook.exits.size() == 1, "world-open callbacks not once");
        check(Hook.entries.getFirst().receiver() == receiver && Hook.entries.getFirst().first().equals("save-name")
                && Hook.exits.getFirst().receiver() == receiver && Hook.exits.getFirst().first().equals("save-name"),
                "world-open callbacks lost receiver/save name");

        Object key = new Object();
        Object value = new Object();
        for (String register : new String[] {"blockRegister", "itemRegister"}) {
            Hook.resetObservations();
            int initialBodies = bodies(type);
            type.getMethod(register, int.class, Object.class, Object.class).invoke(null, 120022, key, value);
            check(Hook.calls == 1 && Hook.nativeNumber == 120022 && Hook.nativeFirst == key
                    && Hook.nativeSecond == value, "static native registration argument slots changed");
            check(bodies(type) == initialBodies + 1 && type.getField("staticNumber").getInt(null) == 120022
                    && type.getField("staticFirst").get(null) == key && type.getField("staticSecond").get(null) == value,
                    "static registration original body/argument slots changed");
            check(Hook.entries.size() == 1 && Hook.exits.size() == 1, "static registration callbacks not once");
            for (Hook.Observation observation : java.util.List.of(Hook.entries.getFirst(), Hook.exits.getFirst())) {
                check(observation.receiver() == null && observation.first() == key && observation.second() == value
                        && observation.number() == 120022, "static registration receiver/int/ref slots changed");
            }
        }
        Hook.resetObservations();
        int initialBodies = bodies(type);
        type.getMethod("recipe", Object.class, Object.class).invoke(null, key, value);
        check(Hook.calls == 1 && Hook.nativeFirst == key && Hook.nativeSecond == value
                && bodies(type) == initialBodies + 1, "static recipe native/body arguments changed");
        check(Hook.entries.size() == 1 && Hook.exits.size() == 1, "static recipe callbacks not once");
        for (Hook.Observation observation : java.util.List.of(Hook.entries.getFirst(), Hook.exits.getFirst())) {
            check(observation.receiver() == null && observation.first() == key && observation.second() == value,
                    "static recipe callback reference slots/receiver changed");
        }
        Hook.resetObservations();
        check(type.getMethod("staticState", int.class).invoke(null, 65537) == key, "static reference result changed");
        check(Hook.calls == 1 && Hook.nativeNumber == 65537 && Hook.entries.isEmpty()
                && Hook.exits.size() == 1 && Hook.exits.getFirst().receiver() == null
                && Hook.exits.getFirst().number() == 65537 && Hook.exits.getFirst().result() == key,
                "static slot-zero metadata/reference result lost");
        Hook.resetObservations();
        check(type.getMethod("staticMetadata", Object.class).invoke(null, value).equals(90004), "static integer result changed");
        check(Hook.calls == 1 && Hook.nativeFirst == value && Hook.entries.isEmpty()
                && Hook.exits.size() == 1 && Hook.exits.getFirst().receiver() == null
                && Hook.exits.getFirst().first() == value && Hook.exits.getFirst().resultNumber() == 90004,
                "static state/integer result lost");
        Hook.resetObservations();
        check(type.getMethod("itemMetadata").invoke(receiver).equals(80002), "item integer result narrowed");
        check(Hook.exits.size() == 1 && Hook.exits.getFirst().resultNumber() == 80002, "item integer callback lost");

        Object source = new Object();
        Hook.resetObservations();
        Hook.commandResult = 70003;
        int before = bodies(type);
        check(type.getMethod("command", Object.class, String.class).invoke(receiver, source, "fixture cmd").equals(70003),
                "handled command did not return exact Integer decision");
        check(bodies(type) == before && Hook.commandReceiver == receiver && Hook.commandSource == source
                && Hook.commandText.equals("fixture cmd"), "handled command body/context changed");
        Hook.commandResult = null;
        check(type.getMethod("command", Object.class, String.class).invoke(receiver, source, "vanilla").equals(23)
                && bodies(type) == before + 1, "unhandled command did not execute vanilla body");

        Hook.resetObservations();
        Hook.verdict = true;
        before = bodies(type);
        check(type.getMethod("fire", Object.class, float.class).invoke(receiver, source, 1.25f).equals(false),
                "fire refusal did not return false");
        check(bodies(type) == before && Hook.refusalReceiver == receiver && Hook.refusalSource == source,
                "fire refusal ran vanilla body or lost damage identity");
        Hook.verdict = false;
        check(type.getMethod("fire", Object.class, float.class).invoke(receiver, source, 1.25f).equals(true)
                && bodies(type) == before + 1, "allowed boolean return/body changed");

        Hook.resetObservations();
        before = bodies(type);
        try {
            type.getMethod("throwing", openParameters).invoke(receiver, "throw-save", "display", seed, generator, "settings");
            throw new AssertionError("rewritten throwing body returned normally");
        } catch (InvocationTargetException failure) {
            check(failure.getCause() instanceof IllegalStateException
                    && failure.getCause().getMessage().equals("fixture body failure"), "original exception changed");
        }
        check(bodies(type) == before + 1 && Hook.entries.size() == 1 && Hook.exits.isEmpty(),
                "throwing method ran success-exit callback or skipped entry/body");
        System.out.println("legacy executable transformation constructors=verified wide-slots=verified typed-results=verified refusal=verified throws=verified");
    }

    private static int bodies(Class<?> type) throws Exception {
        return type.getField("bodies").getInt(null);
    }
}
