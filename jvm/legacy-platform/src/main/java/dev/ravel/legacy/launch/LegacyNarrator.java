package dev.ravel.legacy.launch;

import java.lang.classfile.ClassFile;
import java.lang.classfile.MethodModel;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;

/** Optional text-to-speech stays inactive: no unnamed module receives native access. */
public final class LegacyNarrator {
    private static final ClassDesc NARRATOR = ClassDesc.of("com.mojang.text2speech.Narrator");
    private static final ClassDesc DUMMY = ClassDesc.of("com.mojang.text2speech.NarratorDummy");
    private static final ClassDesc HELPER = ClassDesc.of("dev.ravel.legacy.launch.LegacyNarrator");
    private static final MethodTypeDesc VOID = MethodTypeDesc.ofDescriptor("()V");
    private static boolean logged;

    private LegacyNarrator() {}

    static byte[] transform(byte[] original) {
        var classFile = ClassFile.of();
        var model = classFile.parse(original);
        if (!model.thisClass().asSymbol().equals(NARRATOR)) {
            throw new IllegalArgumentException("Narrator transform requires the pinned narrator interface");
        }
        long factories = model.methods().stream().filter(LegacyNarrator::isFactory).count();
        if (factories != 1) throw new IllegalArgumentException("Expected one narrator factory, found " + factories);
        return classFile.transformClass(model, (builder, element) -> {
            if (element instanceof MethodModel method && isFactory(method)) {
                builder.withMethod("getNarrator", MethodTypeDesc.of(NARRATOR),
                        ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, methodBuilder -> methodBuilder.withCode(code ->
                                code.invokestatic(HELPER, "disabled", VOID)
                                        .new_(DUMMY).dup().invokespecial(DUMMY, "<init>", VOID).areturn()));
            } else {
                builder.with(element);
            }
        });
    }

    private static boolean isFactory(MethodModel method) {
        return method.methodName().equalsString("getNarrator")
                && method.methodType().equalsString("()Lcom/mojang/text2speech/Narrator;");
    }

    public static synchronized void disabled() {
        if (logged) return;
        logged = true;
        System.out.println("RAVEL_PLATFORM narrator=disabled reason=native-access-bounded-to-lwjgl");
    }
}
