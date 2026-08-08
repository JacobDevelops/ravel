import dev.ravel.jvm.refusal.RefusalTarget;

/** Calls a target whose prologue may refuse it, and reports whether its body ran. */
public final class RefusalMain {
    public static void main(String[] args) {
        RefusalTarget.record();
        System.out.println("bodies=" + RefusalTarget.bodies()
                + " methods=" + dev.ravel.jvm.Agent.rewrittenMethods());
    }
}
