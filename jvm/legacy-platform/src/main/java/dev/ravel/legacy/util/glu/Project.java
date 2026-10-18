package dev.ravel.legacy.util.glu;

public final class Project {
    private Project() {}
    public static void gluPerspective(float fovy, float aspect, float near, float far) {
        double halfAngle=Math.toRadians(fovy)*0.5;
        if (far==near || aspect==0 || Math.sin(halfAngle)==0) return;
        double top=near*Math.tan(halfAngle);
        org.lwjgl.opengl.GL11.glFrustum(-top*aspect,top*aspect,-top,top,near,far);
    }
}
