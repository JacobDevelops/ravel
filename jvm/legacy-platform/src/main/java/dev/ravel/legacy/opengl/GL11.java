package dev.ravel.legacy.opengl;

/** Independently written adapters for the pinned vanilla constant-pool surface. */
public final class GL11 extends org.lwjgl.opengl.GL11 {
    private GL11() {}
    public static void glDrawArrays(int mode, int first, int count) {
        if (!CompositeFrame.draw(mode, first, count))
            org.lwjgl.opengl.GL11.glDrawArrays(mode, first, count);
    }
    public static void glFog(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL11.glFogfv(a0, a1);
    }
    public static void glGetFloat(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL11.glGetFloatv(a0, a1);
    }
    public static void glGetInteger(int a0, java.nio.IntBuffer a1) {
        org.lwjgl.opengl.GL11.glGetIntegerv(a0, a1);
    }
    public static void glLight(int a0, int a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL11.glLightfv(a0, a1, a2);
    }
    public static void glLightModel(int a0, java.nio.FloatBuffer a1) {
        org.lwjgl.opengl.GL11.glLightModelfv(a0, a1);
    }
    public static void glMultMatrix(java.nio.FloatBuffer a0) {
        org.lwjgl.opengl.GL11.glMultMatrixf(a0);
    }
    public static void glTexEnv(int a0, int a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL11.glTexEnvfv(a0, a1, a2);
    }
    public static void glTexGen(int a0, int a1, java.nio.FloatBuffer a2) {
        org.lwjgl.opengl.GL11.glTexGenfv(a0, a1, a2);
    }
}
