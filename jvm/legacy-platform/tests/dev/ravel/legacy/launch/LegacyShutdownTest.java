package dev.ravel.legacy.launch;

import java.lang.classfile.ClassFile;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.util.concurrent.CountDownLatch;

/** Standalone shutdown contracts; no game jar, display, or native context is required. */
public final class LegacyShutdownTest {
    private LegacyShutdownTest() {}

    public static void main(String[] args) throws Exception {
        deadThreadReturns();
        unstartedThreadReturns();
        liveThreadTimesOut();
        selfThreadFails();
        nullOwnerThreadReturns();
        System.out.println("RAVEL_SHUTDOWN dead=true unstarted=true timeout=true self=true null=true");
    }

    private static void deadThreadReturns() throws InterruptedException {
        Thread thread = new Thread(() -> {}, "legacy-shutdown-dead");
        thread.start();
        thread.join();
        LegacyShutdown.await(thread, 25L);
        if (thread.isAlive()) throw new AssertionError("terminated thread must remain terminated");
    }

    private static void unstartedThreadReturns() throws InterruptedException {
        Thread thread = new Thread(() -> {}, "legacy-shutdown-unstarted");
        LegacyShutdown.await(thread, 25L);
        if (thread.getState() != Thread.State.NEW) {
            throw new AssertionError("await must not start the owner thread");
        }
    }

    private static void liveThreadTimesOut() throws InterruptedException {
        CountDownLatch release = new CountDownLatch(1);
        Thread thread = new Thread(() -> {
            try {
                release.await();
            } catch (InterruptedException interrupted) {
                throw new AssertionError("shutdown wait must not interrupt the owner", interrupted);
            }
        }, "legacy-shutdown-live");
        thread.setDaemon(true);
        thread.start();
        try {
            long start = System.nanoTime();
            try {
                LegacyShutdown.await(thread, 25L);
                throw new AssertionError("live owner must time out");
            } catch (IllegalStateException expected) {
                if (!expected.getMessage().contains("did not terminate")) throw expected;
            }
            long elapsed = System.nanoTime() - start;
            if (elapsed < 25_000_000L || elapsed > 5_000_000_000L) {
                throw new AssertionError("shutdown wait did not respect its bound: " + elapsed);
            }
            if (!thread.isAlive()) throw new AssertionError("timeout must not stop the owner");
        } finally {
            release.countDown();
            thread.join(1_000L);
        }
        if (thread.isAlive()) throw new AssertionError("test owner failed to finish");
    }

    private static void selfThreadFails() throws InterruptedException {
        try {
            LegacyShutdown.await(Thread.currentThread(), 25L);
            throw new AssertionError("owner must not await itself");
        } catch (IllegalStateException expected) {
            if (!expected.getMessage().contains("own thread")) throw expected;
        }
    }

    private static void nullOwnerThreadReturns() throws ReflectiveOperationException,
            InterruptedException {
        // Isolate the exact vanilla owner name instead of shadowing the real game on the test classpath.
        byte[] fixture = ClassFile.of().build(ClassDesc.of("net.minecraft.server.MinecraftServer"),
                builder -> builder.withFlags(ClassFile.ACC_PUBLIC)
                        .withField("aa", ClassDesc.of("java.lang.Thread"), ClassFile.ACC_PRIVATE)
                        .withMethodBody("<init>", MethodTypeDesc.of(ConstantDescs.CD_void),
                                ClassFile.ACC_PUBLIC, code -> code.aload(0)
                                        .invokespecial(ConstantDescs.CD_Object, "<init>",
                                                MethodTypeDesc.of(ConstantDescs.CD_void))
                                        .return_()));
        Class<?> owner = new ClassLoader(LegacyShutdownTest.class.getClassLoader()) {
            Class<?> defineOwner() {
                return defineClass(null, fixture, 0, fixture.length);
            }
        }.defineOwner();
        LegacyShutdown.await(owner.getConstructor().newInstance());
    }
}
