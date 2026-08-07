package dev.ravel.jvm.hierarchy;

public final class HierarchySource {
    private HierarchySource() {
    }

    public static HierarchyBase choose(boolean target) {
        HierarchyBase selected;
        if (target) {
            selected = new HierarchyTarget();
        } else {
            selected = new HierarchySibling();
        }
        return selected;
    }
}
