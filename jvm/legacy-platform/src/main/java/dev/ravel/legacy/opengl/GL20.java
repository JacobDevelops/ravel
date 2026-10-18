package dev.ravel.legacy.opengl;

/** Independently written adapters for the pinned vanilla constant-pool surface. */
public final class GL20 extends org.lwjgl.opengl.GL20 {
    private GL20() {}
    public static void glShaderSource(int a0, java.nio.ByteBuffer a1) {
        org.lwjgl.opengl.GL20.glShaderSource(a0, org.lwjgl.system.MemoryUtil.memUTF8(a1));
    }
    public static void glUniform1(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform1fv(a0, a1);
    }
    public static void glUniform1(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform1iv(a0, a1);
    }
    public static void glUniform2(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform2fv(a0, a1);
    }
    public static void glUniform2(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform2iv(a0, a1);
    }
    public static void glUniform3(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform3fv(a0, a1);
    }
    public static void glUniform3(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform3iv(a0, a1);
    }
    public static void glUniform4(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform4fv(a0, a1);
    }
    public static void glUniform4(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.GL20.glUniform4iv(a0, a1);
    }
    public static void glUniformMatrix2(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL20.glUniformMatrix2fv(a0, a1, a2);
    }
    public static void glUniformMatrix3(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL20.glUniformMatrix3fv(a0, a1, a2);
    }
    public static void glUniformMatrix4(int a0, boolean a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL20.glUniformMatrix4fv(a0, a1, a2);
    }
}
