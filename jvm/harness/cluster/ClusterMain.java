import dev.ravel.jvm.Hook;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Drives one installed set through a play, save, exit and reload.
 *
 * <p>The set is not this driver's: it is whatever the launch discovered in the
 * game directory's packages tree, so what runs here is the composition a player
 * would get rather than a list written beside the assertions. Both halves of the
 * world round trip cross into Ravel through the two persistence exports the game
 * itself calls, which is why this driver names no game class and needs no
 * generated name: what it stands in for is the world, not the loader.
 *
 * <p>The steps are two processes on purpose. A world reopened in the process
 * that wrote it proves nothing about what reached the disk, so the first process
 * plays, saves and exits, and the second is handed only the file the first left
 * behind.
 */
public final class ClusterMain {

    private ClusterMain() {
    }

    /** Tag byte the save crossing puts in front of a replacement. */
    private static final byte REPLACEMENT = 0x01;

    /**
     * Runs one half of the scenario.
     *
     * @param arguments {@code play} or {@code reload}, then the record file
     * @throws IOException when the record file cannot be read or written
     */
    public static void main(String[] arguments) throws IOException {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("a cluster half and a record file are named");
        }
        Path record = Path.of(arguments[1]);
        // Absolute because the world identity a launch hands over is the game's
        // own path, and the write side refuses a relative one.
        Path world = Path.of("world").toAbsolutePath();
        Files.createDirectories(world);
        String identity = world.toString();
        if ("play".equals(arguments[0])) {
            play(identity, record);
        } else if ("reload".equals(arguments[0])) {
            reload(identity, record);
        } else {
            throw new IllegalArgumentException("no cluster half is called " + arguments[0]);
        }
    }

    /** The three steps a first visit runs: open a fresh world, save it, leave. */
    private static void play(String identity, Path record) throws IOException {
        String refusal = Hook.ravelOpenWorldPersistence(null, identity, true);
        if (refusal == null || !refusal.isEmpty()) {
            step("fresh-play", "failed", "refusal=" + refusal);
            System.exit(1);
        }
        step("fresh-play", "passed", "mods=" + installed());

        byte[] answer = Hook.ravelSaveWorldPersistence(identity);
        if (answer == null || answer.length == 0 || answer[0] != REPLACEMENT) {
            step("save", "failed", "answer=" + tag(answer));
            System.exit(1);
        }
        byte[] aggregate = new byte[answer.length - 1];
        System.arraycopy(answer, 1, aggregate, 0, aggregate.length);
        Files.write(record, aggregate);
        step("save", "passed", "record=written");

        // Leaving is what makes the record the only thing the next visit has, so
        // the step is about the file rather than about the value still in memory.
        byte[] stored = Files.readAllBytes(record);
        if (!java.util.Arrays.equals(stored, aggregate)) {
            step("exit", "failed", "record=truncated");
            System.exit(1);
        }
        step("exit", "passed", "record=captured");
    }

    /** The second visit, handed nothing but the file the first one left. */
    private static void reload(String identity, Path record) throws IOException {
        byte[] stored = Files.readAllBytes(record);
        String refusal = Hook.ravelOpenWorldPersistence(stored, identity, false);
        if (refusal == null || !refusal.isEmpty()) {
            step("reload", "failed", "refusal=" + refusal);
            System.exit(1);
        }
        byte[] answer = Hook.ravelSaveWorldPersistence(identity);
        if (answer == null || answer.length == 0 || answer[0] != REPLACEMENT) {
            step("reload", "failed", "answer=" + tag(answer));
            System.exit(1);
        }
        byte[] round = new byte[answer.length - 1];
        System.arraycopy(answer, 1, round, 0, round.length);
        // What the second visit would now store, against what the first one did.
        // A visit that ignored the record it was handed and initialized fresh
        // state would write the first visit's bytes back unchanged, because that
        // is what a fresh world produces; so equality here is the failure and a
        // successor is the pass. Which successor is the mods' own business and is
        // not decoded here. That the successor is a function of the record and
        // nothing else is the second run's subject rather than this one's.
        if (java.util.Arrays.equals(stored, round)) {
            step("reload", "failed", "record=unread");
            System.exit(1);
        }
        Files.write(Path.of(record + ".reloaded"), round);
        step("reload", "passed", "mods=" + installed() + " record=advanced");
    }

    /** How many mods the launch's own surface holds, which is the discovered set. */
    private static int installed() {
        String surface = Hook.ravelModSurface();
        if (surface == null || surface.isEmpty()) {
            return -1;
        }
        // The wire is a run of length-prefixed fields and the fourth is the row
        // count, which is the whole of what this driver reads from it.
        int at = 0;
        String field = "";
        for (int index = 0; index < 4; index++) {
            int colon = surface.indexOf(':', at);
            int length = Integer.parseInt(surface.substring(at, colon));
            at = colon + 1;
            field = surface.substring(at, at + length);
            at += length;
        }
        return Integer.parseInt(field);
    }

    /** What a save answer was, for a step that did not get the one it needed. */
    private static String tag(byte[] answer) {
        if (answer == null) {
            return "null";
        }
        if (answer.length == 0) {
            return "empty";
        }
        return Integer.toString(answer[0]);
    }

    /** One step's report line, in the form the section compares. */
    private static void step(String name, String result, String detail) {
        System.out.println("[ravel] cluster " + name + "=" + result + " " + detail);
    }
}
