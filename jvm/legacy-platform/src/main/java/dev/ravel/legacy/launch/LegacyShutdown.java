package dev.ravel.legacy.launch;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeElement;
import java.lang.classfile.MethodModel;
import java.lang.classfile.Opcode;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;

/** Orders the pinned vanilla client's unload after the integrated server's final save. */
public final class LegacyShutdown {
    private static final ClassDesc CLIENT = ClassDesc.of("bib");
    private static final ClassDesc SERVER = ClassDesc.of("chd");
    private static final ClassDesc HELPER = ClassDesc.of("dev.ravel.legacy.launch.LegacyShutdown");
    private static final MethodTypeDesc AWAIT = MethodTypeDesc.ofDescriptor("(Ljava/lang/Object;)V");

    private LegacyShutdown() {}

    static byte[] transform(byte[] original, ClassLoader loader) {
        var classFile = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                ClassHierarchyResolver.ofResourceParsing(loader).cached()));
        var model = classFile.parse(original);
        if (!model.thisClass().asSymbol().equals(CLIENT)) {
            throw new IllegalArgumentException("Shutdown transform requires bib");
        }
        long methods = model.methods().stream().filter(LegacyShutdown::isUnload).count();
        if (methods != 1) {
            throw new IllegalArgumentException("Expected exactly one bib.a world unload, found " + methods);
        }
        int[] calls = {0};
        byte[] transformed = classFile.transformClass(model,
                ClassTransform.transformingMethodBodies(LegacyShutdown::isUnload, (builder, element) -> {
                    builder.with(element);
                    if (isStop(element)) {
                        calls[0]++;
                        // x() only requests stop; closing the cache now races asynchronous save IO.
                        builder.aload(0).getfield(CLIENT, "al", SERVER)
                                .invokestatic(HELPER, "await", AWAIT);
                    }
                }));
        if (calls[0] != 1) {
            throw new IllegalArgumentException("Expected exactly one chd.x()V stop request, found " + calls[0]);
        }
        return transformed;
    }

    private static boolean isUnload(MethodModel method) {
        return method.methodName().equalsString("a")
                && method.methodType().equalsString("(Lbsb;Ljava/lang/String;)V");
    }

    private static boolean isStop(CodeElement element) {
        return element instanceof InvokeInstruction invoke
                && invoke.opcode() == Opcode.INVOKEVIRTUAL
                && invoke.owner().asSymbol().equals(SERVER)
                && invoke.name().equalsString("x") && invoke.type().equalsString("()V");
    }

    public static void await(Object server) throws InterruptedException {
        await(serverThread(server), 120_000L);
    }

    static void await(Thread thread, long timeoutMillis) throws InterruptedException {
        if (thread == null) return;
        if (thread == Thread.currentThread()) {
            throw new IllegalStateException("Cannot await integrated server shutdown from its own thread");
        }
        thread.join(timeoutMillis);
        if (thread.isAlive()) {
            throw new IllegalStateException("Integrated server did not terminate within "
                    + timeoutMillis + " milliseconds");
        }
    }

    private static Thread serverThread(Object server) {
        if (server == null) throw new IllegalArgumentException("Integrated server is missing");
        Class<?> type = server.getClass();
        while (type != null && !type.getName().equals("net.minecraft.server.MinecraftServer")) {
            type = type.getSuperclass();
        }
        if (type == null) throw new IllegalArgumentException("Not a vanilla MinecraftServer");
        try {
            var field = type.getDeclaredField("aa");
            if (field.getType() != Thread.class) {
                throw new IllegalStateException("MinecraftServer.aa is not a Thread");
            }
            field.setAccessible(true);
            return (Thread) field.get(server);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Cannot access MinecraftServer.aa shutdown thread", failure);
        }
    }
}
