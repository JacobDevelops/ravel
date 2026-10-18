package dev.ravel.legacy.util.vector;

public class Matrix4f extends Matrix {
    public float m00,m01,m02,m03;
    public float m10,m11,m12,m13;
    public float m20,m21,m22,m23;
    public float m30,m31,m32,m33;
    public Matrix4f() { m00 = m11 = m22 = m33 = 1; }
    @Override public Matrix4f setIdentity() {
        m00 = 1;
        m01 = 0;
        m02 = 0;
        m03 = 0;
        m10 = 0;
        m11 = 1;
        m12 = 0;
        m13 = 0;
        m20 = 0;
        m21 = 0;
        m22 = 1;
        m23 = 0;
        m30 = 0;
        m31 = 0;
        m32 = 0;
        m33 = 1;
        return this;
    }
    public Matrix transpose() {
        float value = m01; m01 = m10; m10 = value;
        value = m02; m02 = m20; m20 = value;
        value = m03; m03 = m30; m30 = value;
        value = m12; m12 = m21; m21 = value;
        value = m13; m13 = m31; m31 = value;
        value = m23; m23 = m32; m32 = value;
        return this;
    }
    public Matrix invert() {
        // Singular inversion returns before any mutation; adjugate entries precede writes.
        float n00 = minor(m11,m21,m31, m12,m22,m32, m13,m23,m33);
        float n01 = -minor(m01,m21,m31, m02,m22,m32, m03,m23,m33);
        float n02 = minor(m01,m11,m31, m02,m12,m32, m03,m13,m33);
        float n03 = -minor(m01,m11,m21, m02,m12,m22, m03,m13,m23);
        float determinant = m00*n00 + m10*n01 + m20*n02 + m30*n03;
        if (determinant == 0) return null;
        float n10 = -minor(m10,m20,m30, m12,m22,m32, m13,m23,m33);
        float n11 = minor(m00,m20,m30, m02,m22,m32, m03,m23,m33);
        float n12 = -minor(m00,m10,m30, m02,m12,m32, m03,m13,m33);
        float n13 = minor(m00,m10,m20, m02,m12,m22, m03,m13,m23);
        float n20 = minor(m10,m20,m30, m11,m21,m31, m13,m23,m33);
        float n21 = -minor(m00,m20,m30, m01,m21,m31, m03,m23,m33);
        float n22 = minor(m00,m10,m30, m01,m11,m31, m03,m13,m33);
        float n23 = -minor(m00,m10,m20, m01,m11,m21, m03,m13,m23);
        float n30 = -minor(m10,m20,m30, m11,m21,m31, m12,m22,m32);
        float n31 = minor(m00,m20,m30, m01,m21,m31, m02,m22,m32);
        float n32 = -minor(m00,m10,m30, m01,m11,m31, m02,m12,m32);
        float n33 = minor(m00,m10,m20, m01,m11,m21, m02,m12,m22);
        float reciprocal = 1 / determinant;
        m00 = n00 * reciprocal;
        m01 = n01 * reciprocal;
        m02 = n02 * reciprocal;
        m03 = n03 * reciprocal;
        m10 = n10 * reciprocal;
        m11 = n11 * reciprocal;
        m12 = n12 * reciprocal;
        m13 = n13 * reciprocal;
        m20 = n20 * reciprocal;
        m21 = n21 * reciprocal;
        m22 = n22 * reciprocal;
        m23 = n23 * reciprocal;
        m30 = n30 * reciprocal;
        m31 = n31 * reciprocal;
        m32 = n32 * reciprocal;
        m33 = n33 * reciprocal;
        return this;
    }
    private static float minor(float a, float b, float c, float d, float e, float f,
                               float g, float h, float i) {
        return a * (e*i - f*h) - b * (d*i - f*g) + c * (d*h - e*g);
    }
    public static Matrix4f mul(Matrix4f a, Matrix4f b, Matrix4f out) {
        if (out == null) out = new Matrix4f();
        float n00 = a.m00 * b.m00 + a.m10 * b.m01 + a.m20 * b.m02 + a.m30 * b.m03;
        float n01 = a.m01 * b.m00 + a.m11 * b.m01 + a.m21 * b.m02 + a.m31 * b.m03;
        float n02 = a.m02 * b.m00 + a.m12 * b.m01 + a.m22 * b.m02 + a.m32 * b.m03;
        float n03 = a.m03 * b.m00 + a.m13 * b.m01 + a.m23 * b.m02 + a.m33 * b.m03;
        float n10 = a.m00 * b.m10 + a.m10 * b.m11 + a.m20 * b.m12 + a.m30 * b.m13;
        float n11 = a.m01 * b.m10 + a.m11 * b.m11 + a.m21 * b.m12 + a.m31 * b.m13;
        float n12 = a.m02 * b.m10 + a.m12 * b.m11 + a.m22 * b.m12 + a.m32 * b.m13;
        float n13 = a.m03 * b.m10 + a.m13 * b.m11 + a.m23 * b.m12 + a.m33 * b.m13;
        float n20 = a.m00 * b.m20 + a.m10 * b.m21 + a.m20 * b.m22 + a.m30 * b.m23;
        float n21 = a.m01 * b.m20 + a.m11 * b.m21 + a.m21 * b.m22 + a.m31 * b.m23;
        float n22 = a.m02 * b.m20 + a.m12 * b.m21 + a.m22 * b.m22 + a.m32 * b.m23;
        float n23 = a.m03 * b.m20 + a.m13 * b.m21 + a.m23 * b.m22 + a.m33 * b.m23;
        float n30 = a.m00 * b.m30 + a.m10 * b.m31 + a.m20 * b.m32 + a.m30 * b.m33;
        float n31 = a.m01 * b.m30 + a.m11 * b.m31 + a.m21 * b.m32 + a.m31 * b.m33;
        float n32 = a.m02 * b.m30 + a.m12 * b.m31 + a.m22 * b.m32 + a.m32 * b.m33;
        float n33 = a.m03 * b.m30 + a.m13 * b.m31 + a.m23 * b.m32 + a.m33 * b.m33;
        out.m00 = n00;
        out.m01 = n01;
        out.m02 = n02;
        out.m03 = n03;
        out.m10 = n10;
        out.m11 = n11;
        out.m12 = n12;
        out.m13 = n13;
        out.m20 = n20;
        out.m21 = n21;
        out.m22 = n22;
        out.m23 = n23;
        out.m30 = n30;
        out.m31 = n31;
        out.m32 = n32;
        out.m33 = n33;
        return out;
    }
    public static Vector4f transform(Matrix4f a, Vector4f b, Vector4f out) {
        float x = a.m00 * b.x + a.m10 * b.y + a.m20 * b.z + a.m30 * b.w;
        float y = a.m01 * b.x + a.m11 * b.y + a.m21 * b.z + a.m31 * b.w;
        float z = a.m02 * b.x + a.m12 * b.y + a.m22 * b.z + a.m32 * b.w;
        float w = a.m03 * b.x + a.m13 * b.y + a.m23 * b.z + a.m33 * b.w;
        if (out == null) return new Vector4f(x,y,z,w);
        out.x=x; out.y=y; out.z=z; out.w=w; return out;
    }
    public static Matrix4f rotate(float angle, Vector3f axis, Matrix4f source, Matrix4f out) {
        if (out == null) out = new Matrix4f();
        float c=(float)Math.cos(angle), s=(float)Math.sin(angle), t=1-c;
        float x=axis.x, y=axis.y, z=axis.z;
        float r00 = t*x*x+c;
        float r01 = t*x*y+s*z;
        float r02 = t*x*z-s*y;
        float r10 = t*x*y-s*z;
        float r11 = t*y*y+c;
        float r12 = t*y*z+s*x;
        float r20 = t*x*z+s*y;
        float r21 = t*y*z-s*x;
        float r22 = t*z*z+c;
        float n00 = source.m00 * r00 + source.m10 * r01 + source.m20 * r02;
        float n01 = source.m01 * r00 + source.m11 * r01 + source.m21 * r02;
        float n02 = source.m02 * r00 + source.m12 * r01 + source.m22 * r02;
        float n03 = source.m03 * r00 + source.m13 * r01 + source.m23 * r02;
        float n10 = source.m00 * r10 + source.m10 * r11 + source.m20 * r12;
        float n11 = source.m01 * r10 + source.m11 * r11 + source.m21 * r12;
        float n12 = source.m02 * r10 + source.m12 * r11 + source.m22 * r12;
        float n13 = source.m03 * r10 + source.m13 * r11 + source.m23 * r12;
        float n20 = source.m00 * r20 + source.m10 * r21 + source.m20 * r22;
        float n21 = source.m01 * r20 + source.m11 * r21 + source.m21 * r22;
        float n22 = source.m02 * r20 + source.m12 * r21 + source.m22 * r22;
        float n23 = source.m03 * r20 + source.m13 * r21 + source.m23 * r22;
        out.m30 = source.m30;
        out.m31 = source.m31;
        out.m32 = source.m32;
        out.m33 = source.m33;
        out.m00 = n00;
        out.m01 = n01;
        out.m02 = n02;
        out.m03 = n03;
        out.m10 = n10;
        out.m11 = n11;
        out.m12 = n12;
        out.m13 = n13;
        out.m20 = n20;
        out.m21 = n21;
        out.m22 = n22;
        out.m23 = n23;
        return out;
    }
}
