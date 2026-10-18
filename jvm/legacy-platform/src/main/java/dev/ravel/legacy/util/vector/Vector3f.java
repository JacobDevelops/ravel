package dev.ravel.legacy.util.vector;

public final class Vector3f extends Vector implements ReadableVector3f {
    public float x, y, z;
    public Vector3f() {}
    public Vector3f(float x, float y, float z) { set(x, y, z); }
    public Vector3f(ReadableVector3f other) { this(other.getX(), other.getY(), other.getZ()); }
    public float getX() { return x; }
    public float getY() { return y; }
    public float getZ() { return z; }
    public void set(float x, float y, float z) { this.x=x; this.y=y; this.z=z; }
    @Override public Vector3f scale(float factor) { x*=factor; y*=factor; z*=factor; return this; }
    public static float dot(Vector3f a, Vector3f b) { return a.x*b.x+a.y*b.y+a.z*b.z; }
    public static Vector3f add(Vector3f a, Vector3f b, Vector3f out) {
        if (out==null) out=new Vector3f();
        out.set(a.x+b.x,a.y+b.y,a.z+b.z); return out;
    }
    public static Vector3f sub(Vector3f a, Vector3f b, Vector3f out) {
        if (out==null) out=new Vector3f();
        out.set(a.x-b.x,a.y-b.y,a.z-b.z); return out;
    }
    public static Vector3f cross(Vector3f a, Vector3f b, Vector3f out) {
        if (out==null) out=new Vector3f();
        out.set(a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x); return out;
    }
    @Override public boolean equals(Object other) {
        return other instanceof Vector3f v && x==v.x && y==v.y && z==v.z;
    }
    @Override public int hashCode() {
        return 31*(31*Float.hashCode(x==0?0:x)+Float.hashCode(y==0?0:y))+Float.hashCode(z==0?0:z);
    }
}
