import net.minecraft.util.StringUtil;

public final class InitializationOrderMain {
    public static void main(String[] args) {
        System.out.println("allowed=" + StringUtil.isAllowedChatCharacter(65));
        System.out.println("filtered=" + StringUtil.filterText("a\nb", true));
        System.out.println("simple=" + StringUtil.filterText("a\nb"));
        System.out.println("valid=" + StringUtil.isValidPlayerName("player"));
        System.out.println("transformed=" + dev.ravel.jvm.Agent.transformedClasses()
                + " methods=" + dev.ravel.jvm.Agent.rewrittenMethods());
        System.out.println("installed=" + dev.ravel.jvm.Agent.isInstalled("chatAllowedCharacter")
                + " unknown=" + dev.ravel.jvm.Agent.isInstalled("neverGenerated"));
    }
}
