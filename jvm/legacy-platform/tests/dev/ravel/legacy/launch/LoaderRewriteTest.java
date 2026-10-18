package dev.ravel.legacy.launch;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.classfile.ClassFile;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import dev.ravel.legacy.input.Keyboard;
import dev.ravel.legacy.opengl.Display;

/** Pure rewrite and loader boundaries, including the optional narrator's no-native path. */
public final class LoaderRewriteTest {
    private LoaderRewriteTest() {}

    public static void main(String[] args) throws Exception {
        ownerBoundaries();
        rewrittenClass();
        parentDelegation();
        narratorDisabled(Path.of(args[0]));
        System.out.println("RAVEL_LOADER_REWRITE passed=true");
    }

    private static void ownerBoundaries() {
        equal("dev/ravel/legacy/opengl/Display", LegacyAdapters.map("org/lwjgl/opengl/Display"));
        equal("([Ldev/ravel/legacy/input/Keyboard;)Ldev/ravel/legacy/util/vector/Matrix4f;",
                LegacyAdapters.map("([Lorg/lwjgl/input/Keyboard;)Lorg/lwjgl/util/vector/Matrix4f;"));
        equal("org/lwjgl/opengl/GL11C", LegacyAdapters.map("org/lwjgl/opengl/GL11C"));
        equal("Lorg/lwjgl/opengl/DisplayModeExtra;", LegacyAdapters.map("Lorg/lwjgl/opengl/DisplayModeExtra;"));
        equal("org/lwjgl/system/MemoryUtil", LegacyAdapters.map("org/lwjgl/system/MemoryUtil"));
    }

    private static void rewrittenClass() throws Exception {
        byte[] original = ClassFile.of().build(ClassDesc.of("RemapFixture"), builder -> {
            builder.withFlags(ClassFile.ACC_PUBLIC);
            builder.withField("keys", ClassDesc.ofDescriptor("[Lorg/lwjgl/input/Keyboard;"), ClassFile.ACC_PUBLIC);
            builder.withMethod("time", MethodTypeDesc.ofDescriptor("()J"),
                    ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, method -> method.withCode(code ->
                            code.invokestatic(ClassDesc.of("org.lwjgl.Sys"), "getTime",
                                    MethodTypeDesc.ofDescriptor("()J")).lreturn()));
            builder.withMethod("wide", MethodTypeDesc.ofDescriptor("()J"),
                    ClassFile.ACC_PUBLIC | ClassFile.ACC_STATIC, method -> method.withCode(code ->
                            code.ldc(9_876_543_210L).lreturn()));
        });
        Class<?> fixture = new FixtureLoader().define(LegacyLoader.rewrite(original));
        if (fixture.getField("keys").getType() != Keyboard[].class) throw new AssertionError("Array descriptor not remapped");
        if (!(((Long) fixture.getMethod("time").invoke(null)) >= 0)) throw new AssertionError("Sys owner not remapped");
        equal(9_876_543_210L, fixture.getMethod("wide").invoke(null));
    }

    private static void parentDelegation() throws Exception {
        try (var loader = new LegacyLoader(new URL[0], LoaderRewriteTest.class.getClassLoader())) {
            equal(Display.class, loader.loadClass("dev.ravel.legacy.opengl.Display"));
            equal(org.lwjgl.Version.class, loader.loadClass("org.lwjgl.Version"));
            equal("org.lwjgl", org.lwjgl.Version.class.getModule().getName());
        }
    }

    private static void narratorDisabled(Path jar) throws Exception {
        var captured = new ByteArrayOutputStream();
        PrintStream previous = System.out;
        try (var output = new PrintStream(captured, true, StandardCharsets.UTF_8);
                var loader = new LegacyLoader(new URL[] {jar.toUri().toURL()}, LoaderRewriteTest.class.getClassLoader())) {
            System.setOut(output);
            Class<?> narrator = loader.loadClass("com.mojang.text2speech.Narrator");
            Object first = narrator.getMethod("getNarrator").invoke(null);
            Object second = narrator.getMethod("getNarrator").invoke(null);
            equal("com.mojang.text2speech.NarratorDummy", first.getClass().getName());
            equal(false, narrator.getMethod("active").invoke(first));
            equal(false, narrator.getMethod("active").invoke(second));
        } finally {
            System.setOut(previous);
        }
        equal("RAVEL_PLATFORM narrator=disabled reason=native-access-bounded-to-lwjgl\n",
                captured.toString(StandardCharsets.UTF_8));
    }

    private static void equal(Object expected, Object actual) {
        if (!expected.equals(actual)) throw new AssertionError("Expected " + expected + ", got " + actual);
    }

    private static final class FixtureLoader extends ClassLoader {
        Class<?> define(byte[] bytes) { return defineClass("RemapFixture", bytes, 0, bytes.length); }
    }
}
