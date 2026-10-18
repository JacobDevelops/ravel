package dev.ravel.jvm;

/** Distinct subclasses force the classfile writer to resolve a merge edge. */
public final class HierarchyFixture {
    private HierarchyFixture() { }
    public static class Base {
        public String value() { return "base"; }
    }
    public static final class Left extends Base {
        @Override public String value() { return "left"; }
    }
    public static final class Right extends Base {
        @Override public String value() { return "right"; }
    }
    public static Base choose(boolean left) {
        Base selected;
        if (left) selected = new Left();
        else selected = new Right();
        return selected;
    }
}
