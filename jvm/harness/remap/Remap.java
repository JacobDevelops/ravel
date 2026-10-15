import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

// Restores aliases.txt's written=linked names after compiling unnamed-package fixtures.
// Shared Utf8 entries with non-class uses are refused; bytes after the pool stay unchanged.
public final class Remap {

    private Remap() {}

    public static void main(String[] args) throws IOException {
        if (args.length < 2) {
            System.err.println("usage: Remap <aliases.txt> <class directory>...");
            System.exit(2);
        }
        Map<String, String> aliases = aliases(Path.of(args[0]));
        if (aliases.isEmpty()) {
            return;
        }
        for (int at = 1; at < args.length; at++) {
            Path directory = Path.of(args[at]);
            for (Path file : classFiles(directory)) {
                remap(directory, file, aliases);
            }
        }
    }

    private static Map<String, String> aliases(Path path) throws IOException {
        Map<String, String> aliases = new LinkedHashMap<>();
        for (String line : Files.readAllLines(path)) {
            String trimmed = line.trim();
            if (trimmed.isEmpty()) {
                continue;
            }
            int split = trimmed.indexOf('=');
            if (split < 1 || split == trimmed.length() - 1) {
                refuse(path + " holds '" + trimmed + "', which is not a written=linked pair");
            }
            String written = trimmed.substring(0, split);
            if (aliases.putIfAbsent(written, trimmed.substring(split + 1)) != null) {
                refuse(path + " names '" + written + "' twice, so its linked name is not decided");
            }
        }
        return aliases;
    }

    private static List<Path> classFiles(Path directory) throws IOException {
        List<Path> found = new ArrayList<>();
        try (Stream<Path> walk = Files.walk(directory)) {
            walk.filter(path -> path.toString().endsWith(".class")).sorted().forEach(found::add);
        }
        return found;
    }

    private static void remap(Path root, Path file, Map<String, String> aliases)
            throws IOException {
        byte[] original = Files.readAllBytes(file);
        ByteArrayOutputStream written = new ByteArrayOutputStream(original.length);
        String name;
        try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(original));
                DataOutputStream out = new DataOutputStream(written)) {
            int magic = in.readInt();
            if (magic != 0xCAFEBABE) {
                refuse(file + " does not start with a class file header");
            }
            int minor = in.readUnsignedShort();
            int major = in.readUnsignedShort();

            int count = in.readUnsignedShort();
            String[] utf8 = new String[count];
            byte[][] payload = new byte[count][];
            int[] tags = new int[count];
            for (int index = 1; index < count; index++) {
                int tag = in.readUnsignedByte();
                tags[index] = tag;
                switch (tag) {
                    case 1 -> utf8[index] = in.readUTF();
                    case 7, 8, 16, 19, 20 -> payload[index] = read(in, 2);
                    case 15 -> payload[index] = read(in, 3);
                    case 3, 4, 9, 10, 11, 12, 17, 18 -> payload[index] = read(in, 4);
                    case 5, 6 -> {
                        payload[index] = read(in, 8);
                        index++;
                    }
                    default -> refuse(file + " holds constant pool tag " + tag + " at entry "
                            + index + ", which this pass cannot walk past");
                }
            }
            byte[] rest = in.readAllBytes();

            Marks marks = new Marks(count);
            for (int index = 1; index < count; index++) {
                switch (tags[index]) {
                    case 7 -> marks.named[reference(payload[index], 0)] = true;
                    case 8 -> marks.text(reference(payload[index], 0));
                    case 16, 19, 20 -> marks.other[reference(payload[index], 0)] = true;
                    case 12 -> {
                        marks.other[reference(payload[index], 0)] = true;
                        marks.other[reference(payload[index], 2)] = true;
                    }
                    default -> {}
                }
            }
            new Walk(file, rest, utf8, marks).classFile();

            String[] replaced = new String[count];
            for (int index = 1; index < count; index++) {
                if (tags[index] != 1) {
                    continue;
                }
                replaced[index] = replacement(file, index, utf8[index], aliases, marks);
            }

            out.writeInt(magic);
            out.writeShort(minor);
            out.writeShort(major);
            out.writeShort(count);
            for (int index = 1; index < count; index++) {
                if (tags[index] == 0) {
                    continue;
                }
                out.writeByte(tags[index]);
                if (tags[index] == 1) {
                    out.writeUTF(replaced[index]);
                } else {
                    out.write(payload[index]);
                }
            }
            out.write(rest);
            int thisClass = ((rest[2] & 0xFF) << 8) | (rest[3] & 0xFF);
            name = replaced[reference(payload[thisClass], 0)];
        }

        Path landed = root.resolve(name + ".class");
        if (!landed.equals(file)) {
            if (Files.exists(landed)) {
                refuse(file + " would land on " + landed + ", which another class already holds");
            }
            Files.createDirectories(landed.getParent());
        }
        Files.write(landed, written.toByteArray());
        if (!landed.equals(file)) {
            Files.delete(file);
        }
    }

    /** Refuses ambiguous Utf8 entries rather than guessing which use names a class. */
    private static String replacement(
            Path file, int index, String value, Map<String, String> aliases, Marks marks) {
        String linked = aliases.get(value);
        if (linked != null) {
            if (!marks.named[index]) {
                refuse(file + " spells the written class name '" + value + "' at entry " + index
                        + ", which no class entry points at, so what reaches it there is not a"
                        + " name to put back");
            }
            if (marks.other[index]) {
                refuse(file + " spells the written class name '" + value + "' at entry " + index
                        + ", which a class entry points at and something that is not a class entry"
                        + " points at as well, so one entry serves two meanings");
            }
            return linked;
        }
        String token = token(value, aliases);
        if (!token.equals(value) && marks.text[index]) {
            refuse(file + " holds the text '" + value + "' at entry " + index
                    + ", which this pass rewrote; a name a class spells as text is not a name to"
                    + " put back");
        }
        return token;
    }

    /** Which kinds of reference reach each constant pool entry. */
    private static final class Marks {
        /** A {@code CONSTANT_Class} points here, so the entry is a class's name. */
        private final boolean[] named;

        /** Something that is not a {@code CONSTANT_Class} points here. */
        private final boolean[] other;

        /** Something reads this entry as text rather than as a name or a descriptor. */
        private final boolean[] text;

        private Marks(int count) {
            named = new boolean[count];
            other = new boolean[count];
            text = new boolean[count];
        }

        private void text(int index) {
            other[index] = true;
            text[index] = true;
        }
    }

    /**
     * Reads everything after the constant pool for the references it makes.
     *
     * <p>Nothing here is rewritten — the bytes are copied back untouched. What the
     * walk is for is the question the pool alone cannot answer: whether the entry a
     * class entry points at is also the entry a method, a local, a parameter or a
     * source file is named by. An attribute this does not know the shape of is
     * refused rather than skipped, because a skipped body is a reference that was
     * not seen and the whole point is to see all of them.
     */
    private static final class Walk {
        private final Path file;
        private final byte[] bytes;
        private final String[] utf8;
        private final Marks marks;
        private int at;

        private Walk(Path file, byte[] bytes, String[] utf8, Marks marks) {
            this.file = file;
            this.bytes = bytes;
            this.utf8 = utf8;
            this.marks = marks;
        }

        private void classFile() {
            at = 6;
            skip(u2() * 2);
            members();
            members();
            attributes();
        }

        private void members() {
            int members = u2();
            for (int member = 0; member < members; member++) {
                skip(2);
                marks.other[u2()] = true;
                marks.other[u2()] = true;
                attributes();
            }
        }

        private void attributes() {
            int attributes = u2();
            for (int attribute = 0; attribute < attributes; attribute++) {
                int named = u2();
                marks.other[named] = true;
                int length = u4();
                int end = at + length;
                switch (utf8[named]) {
                    case "Code" -> {
                        skip(4);
                        skip(u4());
                        skip(u2() * 8);
                        attributes();
                    }
                    case "Signature" -> marks.other[u2()] = true;
                    case "SourceFile" -> marks.text(u2());
                    case "InnerClasses" -> {
                        int classes = u2();
                        for (int inner = 0; inner < classes; inner++) {
                            skip(4);
                            named(u2());
                            skip(2);
                        }
                    }
                    case "MethodParameters" -> {
                        int parameters = u1();
                        for (int parameter = 0; parameter < parameters; parameter++) {
                            named(u2());
                            skip(2);
                        }
                    }
                    case "LocalVariableTable", "LocalVariableTypeTable" -> {
                        int locals = u2();
                        for (int local = 0; local < locals; local++) {
                            skip(4);
                            marks.text(u2());
                            marks.other[u2()] = true;
                            skip(2);
                        }
                    }
                    case "Record" -> {
                        int components = u2();
                        for (int component = 0; component < components; component++) {
                            marks.other[u2()] = true;
                            marks.other[u2()] = true;
                            attributes();
                        }
                    }
                    case "ConstantValue",
                            "Exceptions",
                            "LineNumberTable",
                            "StackMapTable",
                            "SourceDebugExtension",
                            "Deprecated",
                            "Synthetic",
                            "EnclosingMethod",
                            "NestHost",
                            "NestMembers",
                            "PermittedSubclasses",
                            "BootstrapMethods" -> {}
                    default -> refuse(file + " carries the attribute '" + utf8[named]
                            + "', whose body this pass cannot walk, so a name it points at would"
                            + " go unseen");
                }
                at = end;
            }
        }

        /** One optional name, which an inner class and a parameter may both leave out. */
        private void named(int index) {
            if (index != 0) {
                marks.text(index);
            }
        }

        private int u1() {
            return bytes[at++] & 0xFF;
        }

        private int u2() {
            return (u1() << 8) | u1();
        }

        private int u4() {
            return (u2() << 16) | u2();
        }

        private void skip(int bytesToSkip) {
            at += bytesToSkip;
        }
    }

    private static int reference(byte[] payload, int offset) {
        return ((payload[offset] & 0xFF) << 8) | (payload[offset + 1] & 0xFF);
    }

    private static byte[] read(DataInputStream in, int bytes) throws IOException {
        byte[] buffer = new byte[bytes];
        in.readFully(buffer);
        return buffer;
    }

    // A pool entry carries a name two ways this can put back: as the whole entry,
    // which is what a class entry points at and what `replacement` decides; and as
    // a class token inside a descriptor or a signature, where the character after
    // it says the token ended. An inner class ends its outer name with a `$`, so
    // that is one of the characters that end it.
    private static String token(String value, Map<String, String> aliases) {
        String result = value;
        // Longest written name first, so `Outer$Inner` is never read as `Outer` then `$Inner`.
        for (Map.Entry<String, String> alias : aliases.entrySet().stream()
                .sorted((a, b) -> Integer.compare(b.getKey().length(), a.getKey().length()))
                .toList()) {
            result = token(result, alias.getKey(), alias.getValue());
        }
        return result;
    }

    private static String token(String value, String from, String to) {
        String needle = "L" + from;
        StringBuilder built = new StringBuilder();
        int copied = 0;
        int at = value.indexOf(needle);
        while (at >= 0) {
            int after = at + needle.length();
            char next = after < value.length() ? value.charAt(after) : ' ';
            if (at >= copied && (next == ';' || next == '<' || next == '.' || next == '$')) {
                built.append(value, copied, at).append('L').append(to);
                copied = after;
            }
            at = value.indexOf(needle, Math.max(at + 1, copied));
        }
        if (copied == 0) {
            return value;
        }
        return built.append(value, copied, value.length()).toString();
    }

    private static void refuse(String reason) {
        System.err.println(reason);
        System.exit(1);
    }
}
