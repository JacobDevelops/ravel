import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/** Standalone regression harness using the real mapping-io parser. */
public final class NormalizeMappingsTest {
    private static final String HEADER = "tiny\t2\t0\tofficial\tintermediary\tnamed\n";
    private static final String FIXTURE = HEADER
            + "c\ta\tx/A\treviewed/A\n"
            + "\tf\tLb;\tf\tmiddleField\treviewedField\n"
            + "\tm\t(Lb;[I)Lb;\tm\tmiddleMethod\treviewedMethod\n"
            + "c\tb\tx/B\treviewed/B\n";

    public static void main(String[] arguments) throws IOException {
        namespaceSelectionAndDescriptors();
        refuses(FIXTURE, "absent", "named", "no namespace absent");
        refuses(FIXTURE, "named", "absent", "no namespace absent");
        refuses(FIXTURE, "named", "named", "must differ");
        refuses("tiny\t2\t0\tofficial\tnamed\tnamed\nc\ta\tA\tB\n",
                "official", "named", "namespace");
        refuses("tiny\t2\t0\tofficial\tofficial\tnamed\nc\ta\ta\tA\n",
                "official", "named", "namespace");
        refuses(HEADER + "c\ta\tx/A\tA\nc\ta\tx/B\tB\n",
                "official", "named", "repeats source class");
        refuses(HEADER + "c\ta\tx/A\tA\n\tf\tI\tf\tx\ty\n\tf\tI\tf\tx\ty\n",
                "official", "named", "repeats source field");
        refuses(HEADER + "c\ta\tx/A\tA\n\tm\t()V\tm\tx\ty\n\tm\t()V\tm\tx\ty\n",
                "official", "named", "repeats source method");
        refuses(HEADER + "c\ta\tx/A\t\n", "official", "named", "missing reviewed class");
        malformedUtf8();
        duplicateMappingEntries();
        System.out.println("NormalizeMappings regression tests passed");
    }

    private static void namespaceSelectionAndDescriptors() throws IOException {
        String named = normalize(FIXTURE.getBytes(StandardCharsets.UTF_8), "official", "named");
        equal("C\treviewed/A\ta\n"
                + "F\treviewed/A\treviewedField\tLreviewed/B;\tf\n"
                + "M\treviewed/A\treviewedMethod\t(Lreviewed/B;[I)Lreviewed/B;\tm\n"
                + "C\treviewed/B\tb\n", named);
        String intermediary = normalize(
                FIXTURE.getBytes(StandardCharsets.UTF_8), "named", "intermediary");
        equal("C\tx/A\treviewed/A\n"
                + "F\tx/A\tmiddleField\tLx/B;\treviewedField\n"
                + "M\tx/A\tmiddleMethod\t(Lx/B;[I)Lx/B;\treviewedMethod\n"
                + "C\tx/B\treviewed/B\n", intermediary);
    }

    private static void refuses(String tiny, String runtime, String reviewed, String detail)
            throws IOException {
        try {
            normalize(tiny.getBytes(StandardCharsets.UTF_8), runtime, reviewed);
        } catch (IOException expected) {
            if (!expected.getMessage().contains(detail)) {
                throw new AssertionError("expected diagnostic containing " + detail, expected);
            }
            return;
        }
        throw new AssertionError("normalizer accepted invalid mappings: " + detail);
    }

    private static void malformedUtf8() throws IOException {
        byte[] valid = (HEADER + "c\ta\tx/A\t").getBytes(StandardCharsets.UTF_8);
        byte[] malformed = new byte[valid.length + 3];
        System.arraycopy(valid, 0, malformed, 0, valid.length);
        malformed[valid.length] = (byte) 0xc3;
        malformed[valid.length + 1] = 0x28;
        malformed[valid.length + 2] = '\n';
        try {
            normalize(malformed, "official", "named");
        } catch (java.nio.charset.CharacterCodingException expected) {
            return;
        }
        throw new AssertionError("normalizer accepted malformed UTF-8");
    }

    private static void duplicateMappingEntries() throws IOException {
        byte[] archive = zip(FIXTURE.getBytes(StandardCharsets.UTF_8),
                "mappings/mappings.tiny", "mappings/mappings.tinX");
        byte[] alias = "mappings/mappings.tinX".getBytes(StandardCharsets.UTF_8);
        int replaced = 0;
        for (int offset = 0; offset <= archive.length - alias.length; offset++) {
            boolean matches = true;
            for (int index = 0; index < alias.length; index++) {
                if (archive[offset + index] != alias[index]) {
                    matches = false;
                    break;
                }
            }
            if (matches) {
                archive[offset + alias.length - 1] = 'y';
                replaced++;
            }
        }
        if (replaced != 2) {
            throw new AssertionError("expected local and central ZIP filenames to be patched");
        }
        try {
            normalizeArchive(archive, "official", "named");
        } catch (IOException expected) {
            if (!expected.getMessage().contains("repeats mappings/mappings.tiny")) {
                throw new AssertionError("expected duplicate ZIP entry diagnostic", expected);
            }
            return;
        }
        throw new AssertionError("normalizer accepted duplicate mappings/mappings.tiny entries");
    }

    private static byte[] zip(byte[] tiny, String... names) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ZipOutputStream zip = new ZipOutputStream(bytes)) {
            for (String name : names) {
                zip.putNextEntry(new ZipEntry(name));
                zip.write(tiny);
                zip.closeEntry();
            }
        }
        return bytes.toByteArray();
    }

    private static String normalize(byte[] tiny, String runtime, String reviewed) throws IOException {
        return normalizeArchive(zip(tiny, "mappings/mappings.tiny"), runtime, reviewed);
    }

    private static String normalizeArchive(byte[] zipped, String runtime, String reviewed)
            throws IOException {
        Path archive = Files.createTempFile("ravel-normalizer-test-", ".jar");
        try {
            Files.write(archive, zipped);
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            PrintStream original = System.out;
            try (PrintStream captured = new PrintStream(bytes, false, StandardCharsets.UTF_8)) {
                System.setOut(captured);
                try {
                    NormalizeMappings.main(new String[] {archive.toString(), runtime, reviewed});
                } finally {
                    System.setOut(original);
                }
            }
            return bytes.toString(StandardCharsets.UTF_8);
        } finally {
            Files.delete(archive);
        }
    }

    private static void equal(String expected, String actual) {
        if (!expected.equals(actual)) {
            throw new AssertionError("normalized mapping differs\nexpected:\n" + expected
                    + "actual:\n" + actual);
        }
    }
}
