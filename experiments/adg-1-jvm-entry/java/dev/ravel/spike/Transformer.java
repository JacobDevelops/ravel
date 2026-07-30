package dev.ravel.spike;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.classfile.MethodTransform;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;

/**
 * Injects one hook call at the head of one pinned method.
 *
 * <p>The transformation uses the JDK's own class-file API, so no external
 * bytecode library is part of the candidate.
 */
public final class Transformer implements ClassFileTransformer {

    private static final ClassDesc HOOK = ClassDesc.of("dev.ravel.spike.Hook");
    private static final MethodTypeDesc CALLBACK =
            MethodTypeDesc.of(ConstantDescs.CD_long, ConstantDescs.CD_long);

    private final String targetInternalName;
    private final String targetMethod;
    private final String callback;

    private volatile boolean transformed;

    public Transformer(String targetBinaryName, String targetMethod, String callback) {
        this.targetInternalName = targetBinaryName.replace('.', '/');
        this.targetMethod = targetMethod;
        this.callback = callback;
    }

    /** True once the pinned target was actually rewritten. */
    public boolean transformed() {
        return transformed;
    }

    @Override
    public byte[] transform(
            Module module,
            ClassLoader loader,
            String internalName,
            Class<?> beingRedefined,
            ProtectionDomain domain,
            byte[] classfile) {
        if (internalName == null || !internalName.equals(targetInternalName)) {
            return null;
        }

        ClassFile context = ClassFile.of();
        byte[] rewritten =
                context.transformClass(
                        context.parse(classfile),
                        ClassTransform.transformingMethods(
                                model -> model.methodName().equalsString(targetMethod),
                                MethodTransform.transformingCode(prependCallback())));
        transformed = true;
        return rewritten;
    }

    private CodeTransform prependCallback() {
        return new CodeTransform() {
            private boolean injected;

            @Override
            public void accept(CodeBuilder builder, CodeElement element) {
                if (!injected) {
                    injected = true;
                    builder.loadConstant(1L);
                    builder.invokestatic(HOOK, callback, CALLBACK);
                    builder.pop2();
                }
                builder.with(element);
            }
        };
    }
}
