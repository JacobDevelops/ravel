package dev.ravel.legacy.opengl;

/** Independently written adapters for the pinned vanilla constant-pool surface. */
public final class ARBShaderObjects extends org.lwjgl.opengl.ARBShaderObjects {
    private ARBShaderObjects() {}
    public static void glShaderSourceARB(int a0, java.nio.ByteBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glShaderSourceARB(a0, org.lwjgl.system.MemoryUtil.memUTF8(a1));
    }
    public static void glUniform1ARB(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform1fvARB(a0, a1);
    }
    public static void glUniform1ARB(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform1ivARB(a0, a1);
    }
    public static void glUniform2ARB(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform2fvARB(a0, a1);
    }
    public static void glUniform2ARB(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform2ivARB(a0, a1);
    }
    public static void glUniform3ARB(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform3fvARB(a0, a1);
    }
    public static void glUniform3ARB(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform3ivARB(a0, a1);
    }
    public static void glUniform4ARB(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform4fvARB(a0, a1);
    }
    public static void glUniform4ARB(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.ARBShaderObjects.glUniform4ivARB(a0, a1);
    }
    public static void glUniformMatrix2ARB(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.ARBShaderObjects.glUniformMatrix2fvARB(a0, a1, a2);
    }
    public static void glUniformMatrix3ARB(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.ARBShaderObjects.glUniformMatrix3fvARB(a0, a1, a2);
    }
    public static void glUniformMatrix4ARB(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.ARBShaderObjects.glUniformMatrix4fvARB(a0, a1, a2);
    }
}
