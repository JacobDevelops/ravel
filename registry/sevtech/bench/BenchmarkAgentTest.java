package dev.ravel.metrics;

import java.io.BufferedWriter;
import java.io.StringWriter;
import java.lang.instrument.ClassFileTransformer;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Label;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

/** Verifies the actual observer output without invoking game automation. */
public final class BenchmarkAgentTest {
    private BenchmarkAgentTest() { }

    public static void main(String[] arguments) throws Exception {
        byte[] original = monitorCleanup();
        new FixtureLoader().define(original).getDeclaredMethod("az");
        StringWriter events = new StringWriter();
        Field eventWriter = BenchmarkAgent.class.getDeclaredField("events");
        eventWriter.setAccessible(true);
        Object previous = eventWriter.get(null);
        try {
            eventWriter.set(null, new BufferedWriter(events));
            Constructor<?> constructor = Class.forName(
                "dev.ravel.metrics.BenchmarkAgent$ObserverTransformer").getDeclaredConstructor();
            constructor.setAccessible(true);
            ClassFileTransformer transformer = (ClassFileTransformer) constructor.newInstance();
            FixtureLoader loader = new FixtureLoader();
            byte[] instrumented = transformer.transform(loader, "bib", null, null, original);
            if (instrumented == null) {
                throw new AssertionError("Observer did not instrument bib.az");
            }
            // Resolving methods forces JVM verification, but never runs frameEnter/automation.
            Class<?> fixture = loader.define(instrumented);
            fixture.getDeclaredMethod("az");
            if (!events.toString().contains("hooks_transformed")) {
                throw new AssertionError("Actual transformer did not report its hook");
            }
            verifyDiscardedLocals(transformer, "bxr", "b", "(FFFLbxl;)V", "java/lang/Object", "bxl");
            verifyDiscardedLocals(transformer, "bxm", "a", "(Lbuk;Lcdz;)V",
                "java/lang/Object", "buk", "cdz");
            verifyDiscardedLocals(transformer, "chd", "C", "()V",
                "net/minecraft/server/MinecraftServer", "net/minecraft/server/MinecraftServer");
            verifyDiscardedLocals((ClassFileTransformer) constructor.newInstance(),
                "bib", "az", "()V", "java/lang/Object");
        } finally {
            eventWriter.set(null, previous);
        }
        System.out.println("BenchmarkAgent verifier regression passed");
    }

    private static byte[] monitorCleanup() {
        ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
        writer.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, "bib", null, "java/lang/Object", null);
        MethodVisitor method = writer.visitMethod(Opcodes.ACC_PUBLIC, "az", "()V", null, null);
        Label start = new Label();
        Label end = new Label();
        Label cleanup = new Label();
        method.visitCode();
        method.visitVarInsn(Opcodes.ALOAD, 0);
        method.visitInsn(Opcodes.DUP);
        method.visitVarInsn(Opcodes.ASTORE, 1);
        method.visitInsn(Opcodes.MONITORENTER);
        method.visitLabel(start);
        method.visitInsn(Opcodes.ACONST_NULL);
        method.visitInsn(Opcodes.ATHROW);
        method.visitLabel(end);
        method.visitTryCatchBlock(start, end, cleanup, null);
        method.visitLabel(cleanup);
        // Valid original monitor cleanup explicitly discards the receiver's local.
        // A new encompassing handler cannot require local 0 to remain bib here.
        method.visitFrame(Opcodes.F_FULL, 2, new Object[] { Opcodes.TOP, "java/lang/Object" },
            1, new Object[] { "java/lang/Throwable" });
        method.visitVarInsn(Opcodes.ASTORE, 2);
        method.visitVarInsn(Opcodes.ALOAD, 1);
        method.visitInsn(Opcodes.MONITOREXIT);
        method.visitVarInsn(Opcodes.ALOAD, 2);
        method.visitInsn(Opcodes.ATHROW);
        method.visitMaxs(0, 0);
        method.visitEnd();
        writer.visitEnd();
        return writer.toByteArray();
    }
    private static void verifyDiscardedLocals(ClassFileTransformer transformer, String owner,
            String name, String descriptor, String parent, String... dependencies) throws Exception {
        ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
        writer.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, owner, null, parent, null);
        MethodVisitor method = writer.visitMethod(Opcodes.ACC_PUBLIC, name, descriptor, null, null);
        Label returned = new Label();
        method.visitCode();
        method.visitJumpInsn(Opcodes.GOTO, returned);
        method.visitLabel(returned);
        method.visitFrame(Opcodes.F_FULL, 0, new Object[0], 0, new Object[0]);
        method.visitInsn(Opcodes.RETURN);
        method.visitMaxs(0, 0);
        method.visitEnd();
        writer.visitEnd();
        byte[] original = writer.toByteArray();
        FixtureLoader control = new FixtureLoader();
        FixtureLoader instrumented = new FixtureLoader();
        for (String dependency : dependencies) {
            ClassWriter declaration = new ClassWriter(0);
            declaration.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, dependency, null, "java/lang/Object", null);
            declaration.visitEnd();
            byte[] bytes = declaration.toByteArray();
            control.define(bytes);
            instrumented.define(bytes);
        }
        control.define(original).getDeclaredMethods();
        byte[] observed = transformer.transform(instrumented, owner, null, null, original);
        if (observed == null) {
            throw new AssertionError("Observer did not instrument " + owner + "." + name);
        }
        // The original return has no receiver local: every exit must avoid loading one.
        instrumented.define(observed).getDeclaredMethods();
    }


    private static final class FixtureLoader extends ClassLoader {
        private FixtureLoader() {
            super(BenchmarkAgent.class.getClassLoader());
        }

        private Class<?> define(byte[] bytes) {
            return defineClass(null, bytes, 0, bytes.length);
        }
    }
}
