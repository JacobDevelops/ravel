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

        // The settings screen's two crossings, called with nothing installed.
        // What this proves is the part no Rust test can: that the exported
        // symbols the `jni_mangle` attributes name are the ones the generated
        // declarations resolve to. A mismatch between the two is invisible until
        // a JVM tries to link them, and it would surface in the game rather than
        // here. The document is null because this process holds no configuration,
        // and the save is refused for the same reason — which is itself the answer
        // the screen would show a player.
        System.out.println("settings=" + dev.ravel.jvm.Hook.ravelSettingsDocument("example_mod"));
        System.out.println("saved=" + dev.ravel.jvm.Hook.ravelSaveSettings("1:11:11example_mod1:0"));
        System.out.println("mods=" + dev.ravel.jvm.Hook.ravelModSurface());
    }
}
