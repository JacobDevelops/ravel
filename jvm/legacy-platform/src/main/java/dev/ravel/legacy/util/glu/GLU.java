package dev.ravel.legacy.util.glu;

import java.nio.FloatBuffer;
import java.nio.IntBuffer;

public final class GLU {
    private GLU() {}
    private static final ThreadLocal<double[][]> WORK = ThreadLocal.withInitial(() -> new double[4][5]);

    public static String gluErrorString(int error) {
        return switch (error) {
            case 0 -> "No error";
            case 0x500 -> "Invalid enum";
            case 0x501 -> "Invalid value";
            case 0x502 -> "Invalid operation";
            case 0x503 -> "Stack overflow";
            case 0x504 -> "Stack underflow";
            case 0x505 -> "Out of memory";
            case 0x506 -> "Invalid framebuffer operation";
            case 100900 -> "Invalid enumeration";
            case 100901 -> "Invalid value";
            case 100902 -> "Out of memory";
            default -> null;
        };
    }

    /** Solve projection * model * position = normalized window coordinates with pivoting. */
    public static boolean gluUnProject(float x, float y, float z, FloatBuffer model,
            FloatBuffer projection, IntBuffer viewport, FloatBuffer output) {
        int mp=model.position(), pp=projection.position(), vp=viewport.position();
        int width=viewport.get(vp+2), height=viewport.get(vp+3);
        if (width==0 || height==0) return false;
        double[][] a=WORK.get();
        for (int row=0; row<4; row++) {
            for (int col=0; col<4; col++) {
                double value=0;
                for (int k=0; k<4; k++) value+=projection.get(pp+k*4+row)* (double)model.get(mp+col*4+k);
                a[row][col]=value;
            }
        }
        a[0][4]=2.0*(x-viewport.get(vp))/width-1;
        a[1][4]=2.0*(y-viewport.get(vp+1))/height-1;
        a[2][4]=2.0*z-1;
        a[3][4]=1;
        for (int col=0; col<4; col++) {
            int pivot=col;
            for (int row=col+1; row<4; row++) if (Math.abs(a[row][col])>Math.abs(a[pivot][col])) pivot=row;
            if (a[pivot][col]==0) return false;
            double[] swap=a[col]; a[col]=a[pivot]; a[pivot]=swap;
            double divisor=a[col][col];
            for (int k=col; k<=4; k++) a[col][k]/=divisor;
            for (int row=0; row<4; row++) {
                if (row==col) continue;
                double factor=a[row][col];
                for (int k=col; k<=4; k++) a[row][k]-=factor*a[col][k];
            }
        }
        double w=a[3][4];
        if (w==0) return false;
        int op=output.position();
        output.put(op,(float)(a[0][4]/w));
        output.put(op+1,(float)(a[1][4]/w));
        output.put(op+2,(float)(a[2][4]/w));
        return true;
    }
}
