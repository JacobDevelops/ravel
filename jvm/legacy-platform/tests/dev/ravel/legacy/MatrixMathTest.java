package dev.ravel.legacy;

import java.util.Arrays;
import dev.ravel.legacy.util.vector.Matrix;
import dev.ravel.legacy.util.vector.Matrix4f;

/** Exercise scalar matrix compatibility without a native context. */
public final class MatrixMathTest {
    private MatrixMathTest() {}

    public static void main(String[] args) throws ReflectiveOperationException {
        for (String method : new String[] {"transpose", "invert"}) {
            if (Matrix4f.class.getDeclaredMethod(method).getReturnType() != Matrix.class) {
                throw new AssertionError(method + " must retain its Matrix return descriptor");
            }
        }
        transposeInPlace();
        invertInPlace(affine());
        invertInPlace(projective());
        singularInverseDoesNotMutate();
        System.out.println("RAVEL_MATRIX_MATH transpose=true inverse=true singular=true");
    }

    private static void transposeInPlace() {
        Matrix4f matrix = new Matrix4f();
        matrix.m00 = 1; matrix.m01 = 2; matrix.m02 = 3; matrix.m03 = 4;
        matrix.m10 = 5; matrix.m11 = 6; matrix.m12 = 7; matrix.m13 = 8;
        matrix.m20 = 9; matrix.m21 = 10; matrix.m22 = 11; matrix.m23 = 12;
        matrix.m30 = 13; matrix.m31 = 14; matrix.m32 = 15; matrix.m33 = 16;
        float[] original = elements(matrix);
        if (matrix.transpose() != matrix) {
            throw new AssertionError("transpose must return its receiver");
        }
        float[] transposed = elements(matrix);
        for (int column = 0; column < 4; column++) {
            for (int row = 0; row < 4; row++) {
                if (transposed[column * 4 + row] != original[row * 4 + column]) {
                    throw new AssertionError("transpose layout at column=" + column + " row=" + row);
                }
            }
        }
        if (matrix.transpose() != matrix || !Arrays.equals(original, elements(matrix))) {
            throw new AssertionError("double transpose must restore the receiver exactly");
        }
    }

    private static void invertInPlace(Matrix4f matrix) {
        float[] original = elements(matrix);
        if (matrix.invert() != matrix) {
            throw new AssertionError("invert must return its receiver");
        }
        float[] inverse = elements(matrix);
        for (int column = 0; column < 4; column++) {
            for (int row = 0; row < 4; row++) {
                double product = 0;
                for (int k = 0; k < 4; k++) {
                    product += (double) original[k * 4 + row] * inverse[column * 4 + k];
                }
                double expected = column == row ? 1 : 0;
                if (!(Math.abs(product - expected) <= 0.00001)) {
                    throw new AssertionError("matrix * inverse at column=" + column
                            + " row=" + row + ": expected " + expected + ", got " + product);
                }
            }
        }
    }

    private static void singularInverseDoesNotMutate() {
        Matrix4f matrix = affine();
        matrix.m10 = matrix.m00;
        matrix.m11 = matrix.m01;
        matrix.m12 = matrix.m02;
        matrix.m13 = matrix.m03;
        float[] original = elements(matrix);
        if (matrix.invert() != null) {
            throw new AssertionError("singular invert must return null");
        }
        if (!Arrays.equals(original, elements(matrix))) {
            throw new AssertionError("singular invert must leave every element unchanged");
        }
    }

    private static Matrix4f affine() {
        Matrix4f matrix = new Matrix4f();
        // Z rotation (cos=0.8, sin=0.6), nonuniform scale (2,3,4), and translation.
        matrix.m00 = 1.6f;
        matrix.m01 = 1.2f;
        matrix.m10 = -1.8f;
        matrix.m11 = 2.4f;
        matrix.m22 = 4;
        matrix.m30 = 5;
        matrix.m31 = -7;
        matrix.m32 = 11;
        return matrix;
    }

    private static Matrix4f projective() {
        Matrix4f matrix = new Matrix4f();
        // Dense, nonsymmetric, strictly row-diagonally-dominant projective matrix.
        matrix.m00 = 4; matrix.m01 = -0.75f; matrix.m02 = 0.25f; matrix.m03 = 0.125f;
        matrix.m10 = 0.5f; matrix.m11 = 5; matrix.m12 = -0.5f; matrix.m13 = -0.25f;
        matrix.m20 = -0.25f; matrix.m21 = 0.125f; matrix.m22 = 6; matrix.m23 = 0.375f;
        matrix.m30 = 1; matrix.m31 = -2; matrix.m32 = 3; matrix.m33 = 2;
        return matrix;
    }

    private static float[] elements(Matrix4f matrix) {
        return new float[] {
            matrix.m00, matrix.m01, matrix.m02, matrix.m03,
            matrix.m10, matrix.m11, matrix.m12, matrix.m13,
            matrix.m20, matrix.m21, matrix.m22, matrix.m23,
            matrix.m30, matrix.m31, matrix.m32, matrix.m33
        };
    }
}
