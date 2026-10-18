package dev.ravel.jvm;

import java.io.IOException;
import java.io.InputStream;
import java.lang.classfile.ClassFile;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/** Runs against the isolated fixture Hook, never against substituted game capabilities. */
public final class FixtureMain {
    private FixtureMain() { }
    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    private static final class Definer extends ClassLoader {
        Definer() { super(FixtureMain.class.getClassLoader()); }
        Class<?> define(byte[] bytes) { return defineClass(null, bytes, 0, bytes.length); }
    }
    private static final class Resources extends ClassLoader {
        final Set<String> read = new HashSet<>();
        final String missing;
        Resources(String missing) { super(FixtureMain.class.getClassLoader()); this.missing = missing; }
        @Override public InputStream getResourceAsStream(String name) {
            read.add(name);
            return name.equals(missing) ? null : super.getResourceAsStream(name);
        }
        @Override protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
            if (name.startsWith("dev.ravel.jvm.HierarchyFixture")) {
                throw new AssertionError("hierarchy resolution initialized/loaded " + name);
            }
            return super.loadClass(name, resolve);
        }
    }
    public static void main(String[] arguments) throws ReflectiveOperationException, IOException {
        if (arguments[0].equals("refusal")) refusal();
        else if (arguments[0].equals("descriptors")) descriptorRefusals();
        else hierarchy();
    }
    private static void refusal() throws ReflectiveOperationException {
        String name = "fixture/Refusal";
        byte[] original = ClassFile.of().build(ClassDesc.ofInternalName(name), builder -> {
            builder.withFlags(ClassFile.ACC_PUBLIC);
            builder.withField("bodies", ClassDesc.ofDescriptor("I"), ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC);
            builder.withMethodBody("run", MethodTypeDesc.ofDescriptor("()V"),
                    ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, code -> code
                            .getstatic(ClassDesc.ofInternalName(name), "bodies", ClassDesc.ofDescriptor("I"))
                            .iconst_1().iadd()
                            .putstatic(ClassDesc.ofInternalName(name), "bodies", ClassDesc.ofDescriptor("I"))
                            .return_());
        });
        HookTarget hook = new HookTarget(name, "run", "()V", true, "decide", "()Z", true);
        Transformer transformer = new Transformer(new HookTarget[] {hook}, true);
        byte[] rewritten = transformer.transform(null, FixtureMain.class.getClassLoader(), name, null, null, original);
        require(rewritten != null && transformer.isInstalled("decide"), "refusal prologue not installed");
        Class<?> target = new Definer().define(rewritten);
        Hook.verdict = true;
        target.getMethod("run").invoke(null);
        require(target.getField("bodies").getInt(null) == 0 && Hook.calls == 1, "true did not refuse");
        Hook.verdict = false;
        target.getMethod("run").invoke(null);
        require(target.getField("bodies").getInt(null) == 1 && Hook.calls == 2, "false did not continue");
        System.out.println("legacy refusal true=skipped false=continued");
    }
    private record Drift(String hook, String descriptor, boolean isStatic) {}

    private static void descriptorRefusals() {
        java.util.List<Drift> cases = new java.util.ArrayList<>();
        for (String hook : new String[] {"legacyBlockRegister", "legacyItemRegister"}) {
            cases.add(new Drift(hook, "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V", true));
            cases.add(new Drift(hook, "(ILjava/lang/Object;)V", true));
            cases.add(new Drift(hook, "(ILjava/lang/Object;Ljava/lang/Object;)V", false));
        }
        cases.add(new Drift("legacyBlockStateFromMetadata", "(Ljava/lang/Object;)Ljava/lang/Object;", true));
        cases.add(new Drift("legacyBlockStateFromMetadata", "(I)I", true));
        cases.add(new Drift("legacyBlockMetadataFromState", "(I)I", true));
        cases.add(new Drift("legacyBlockMetadataFromState", "(Ljava/lang/Object;)Ljava/lang/Object;", true));
        for (String hook : new String[] {"legacyPayloadClient", "legacyPayloadServer", "legacyScreenOpen"}) {
            cases.add(new Drift(hook, "()V", false));
            cases.add(new Drift(hook, "(I)V", false));
            cases.add(new Drift(hook, "(Ljava/lang/Object;)V", true));
        }
        cases.add(new Drift("legacyRecipeRegister", "(Ljava/lang/Object;I)V", true));
        cases.add(new Drift("legacyRecipeRegister", "(Ljava/lang/Object;Ljava/lang/Object;)V", false));
        for (Drift drift : cases) {
            // Initialize ConstantDescs before parsing primitive descriptors (JDK 27 initialization order).
            ClassDesc returnType = ConstantDescs.CD_void;
            String owner = "fixture/DescriptorDrift";
            MethodTypeDesc type = MethodTypeDesc.ofDescriptor(drift.descriptor());
            byte[] original = ClassFile.of().build(ClassDesc.ofInternalName(owner), builder -> {
                builder.withFlags(ClassFile.ACC_PUBLIC);
                builder.withMethodBody("run", type,
                        ClassFile.ACC_PUBLIC | (drift.isStatic() ? ClassFile.ACC_STATIC : 0), code -> {
                            switch (java.lang.classfile.TypeKind.from(type.returnType()).asLoadable()) {
                                case VOID -> code.return_();
                                case REFERENCE -> code.aconst_null().areturn();
                                default -> code.iconst_0().ireturn();
                            }
                        });
            });
            HookTarget hook = new HookTarget(owner, "run", drift.descriptor(), drift.isStatic(),
                    drift.hook(), type.changeReturnType(returnType).descriptorString(), false);
            Transformer transformer = new Transformer(new HookTarget[] {hook}, true);
            byte[] snapshot = original.clone();
            byte[] rewritten = transformer.transform(null, FixtureMain.class.getClassLoader(), owner, null, null, original);
            require(rewritten == null && Arrays.equals(original, snapshot) && !transformer.isInstalled(drift.hook())
                    && transformer.rewrittenMethods() == 0 && transformer.transformedClasses() == 0
                    && transformer.transformationFailures().contains(owner.replace('/', '.')),
                    "typed descriptor drift was not refused before publication: " + drift);
        }
        System.out.println("legacy typed descriptor refusals=" + cases.size() + " originals=preserved");
    }
    private static void hierarchy() throws IOException, ReflectiveOperationException {
        String owner = "dev/ravel/jvm/HierarchyFixture";
        byte[] original;
        try (InputStream input = FixtureMain.class.getClassLoader().getResourceAsStream(owner + ".class")) {
            original = java.util.Objects.requireNonNull(input).readAllBytes();
        }
        HookTarget hook = new HookTarget(owner, "choose", "(Z)Ldev/ravel/jvm/HierarchyFixture$Base;",
                true, "probe", "(Z)V", false);
        // The probe must accept the target argument; the fixture Hook has that overload.
        Resources resources = new Resources("");
        Transformer transformer = new Transformer(new HookTarget[] {hook}, true);
        byte[] rewritten = transformer.transform(null, resources, owner, null, null, original);
        require(rewritten != null && transformer.isInstalled("probe"), "hierarchy rewrite failed");
        require(resources.read.contains(owner + "$Left.class") && resources.read.contains(owner + "$Right.class"),
                "merge did not resolve both resources");
        Class<?> target = new Definer().define(rewritten);
        for (boolean left : new boolean[] {true, false}) {
            Object answer = target.getMethod("choose", boolean.class).invoke(null, left);
            require(answer.getClass().getMethod("value").invoke(answer).equals(left ? "left" : "right"),
                    "hierarchy merge changed branch result");
        }
        require(Hook.calls == 2, "hierarchy prologue did not execute");
        for (String missing : new String[] {owner + "$Left.class", owner + "$Right.class", owner + "$Base.class"}) {
            Transformer broken = new Transformer(new HookTarget[] {hook}, true);
            byte[] snapshot = original.clone();
            byte[] result = broken.transform(null, new Resources(missing), owner, null, null, original);
            require(result == null && !broken.isInstalled("probe") && broken.rewrittenMethods() == 0
                    && Arrays.equals(original, snapshot) && !broken.transformationFailures().isEmpty(),
                    "missing hierarchy edge published a rewrite: " + missing);
        }
        System.out.println("legacy hierarchy resource-only branches=2 missing-edges=3");
    }
}
