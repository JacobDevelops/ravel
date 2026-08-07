import dev.ravel.jvm.hierarchy.HierarchyBase;
import dev.ravel.jvm.hierarchy.HierarchySource;

public final class HierarchyResolutionMain {
    public static void main(String[] args) {
        System.out.println("before=" + dev.ravel.jvm.Agent.transformedClasses()
                + " methods=" + dev.ravel.jvm.Agent.rewrittenMethods());
        HierarchyBase selected = HierarchySource.choose(true);
        System.out.println("value=" + selected.value());
        System.out.println("after=" + dev.ravel.jvm.Agent.transformedClasses()
                + " methods=" + dev.ravel.jvm.Agent.rewrittenMethods());
    }
}
