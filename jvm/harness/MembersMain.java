import com.mojang.brigadier.CommandDispatcher;
import dev.ravel.jvm.Agent;
import dev.ravel.jvm.Hook;
import java.lang.reflect.Field;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.core.MappedRegistry;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.SmoothDouble;

/**
 * Drives the four inventory entries that pin a field, against a stand-in game.
 *
 * <p>A field is not a call target. The generator emits no export and no
 * prologue for one — {@code render.rs} skips every hook whose kind is not a
 * method, and {@code ravel-bootstrap-runtime} asserts that no export dispatches through
 * a field's constant — so there is no rewritten field body to run and no
 * crossing to count. What a field entry claims is narrower and is what this
 * proves: the class the transformer actually rewrote still declares that member
 * under the pinned descriptor, and the value read out of it is the one the
 * rewritten sibling method left there.
 *
 * <p>So each line pairs the field with a method target in the same class. The
 * {@code rewrote} marker is the transformer's own installed-prologue set, which
 * a class that merely loaded never enters, and the reflected descriptor is
 * compared against the string {@code hooks.toml} pins character for character.
 *
 * <p>What this does not prove: anything about the real client, which needs a
 * launch.
 */
public final class MembersMain {

    public static void main(String[] args) throws ReflectiveOperationException {
        SmoothDouble smooth = new SmoothDouble();
        smooth.getNewDeltaValue(0.25, 2.0);
        report("input.smooth-last-amount", SmoothDouble.class, "lastAmount", smooth,
                "inputSmoothDelta");

        MappedRegistry registry = new MappedRegistry();
        registry.freeze();
        report("registry.closed", MappedRegistry.class, "frozen", registry, "registryFreeze");

        Commands commands = new Commands();
        commands.performPrefixedCommand(new CommandSourceStack(), "ravel");
        report("command.table", Commands.class, "dispatcher", commands, "commandPerformPrefixed");

        MinecraftServer server = new MinecraftServer();
        server.tickServer(() -> true);
        report("server.tick-count", MinecraftServer.class, "tickCount", server, "serverTick");

        System.out.println("bodies=" + Commands.performed()
                + " hooks=" + Hook.nativeInvocations()
                + " methods=" + Agent.rewrittenMethods());
    }

    /**
     * Prints one pinned field as the transformed class declares it.
     *
     * <p>The command tree is reported by its size rather than by itself: the
     * point of reaching that field is what it holds, and an identity hash is not
     * something an exact-output contract can hold.
     */
    private static void report(
            String id, Class<?> owner, String member, Object instance, String siblingHook)
            throws ReflectiveOperationException {
        Field field = owner.getDeclaredField(member);
        field.setAccessible(true);
        Object value = field.get(instance);
        if (value instanceof CommandDispatcher dispatcher) {
            value = dispatcher.size();
        }
        System.out.println(id + " " + owner.getName() + "." + member + ":"
                + field.getType().descriptorString()
                + " value=" + value
                + " rewrote=" + Agent.isInstalled(siblingHook));
    }
}
