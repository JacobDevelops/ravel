import net.minecraft.util.StringUtil;

// Calls each stubbed target once and then asks the agent what it rewrote. The
// return values are printed because they are the check that the transformer
// prepended a hook call and left the body alone.
public final class Main {
    public static void main(String[] args) {
        System.out.println("allowed=" + StringUtil.isAllowedChatCharacter(65));
        System.out.println("filtered=" + StringUtil.filterText("a\nb", true));
        System.out.println("simple=" + StringUtil.filterText("a\nb"));
        System.out.println("valid=" + StringUtil.isValidPlayerName("jacob"));
        System.out.println("transformed=" + dev.ravel.jvm.Agent.transformedClasses()
                + " methods=" + dev.ravel.jvm.Agent.rewrittenMethods());
    }
}
