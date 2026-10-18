import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import net.fabricmc.mappingio.MappingReader;
import net.fabricmc.mappingio.adapter.ForwardingMappingVisitor;
import net.fabricmc.mappingio.format.MappingFormat;
import net.fabricmc.mappingio.tree.MappingTreeView;
import net.fabricmc.mappingio.tree.MappingTreeView.MemberMappingView;
import net.fabricmc.mappingio.tree.MemoryMappingTree;

/** Build-time Tiny reader. Stdout is only Ravel's normalized mapping protocol. */
public final class NormalizeMappings {
    public static void main(String[] arguments) throws IOException {
        if (arguments.length != 3) {
            throw new IOException("expected mapping jar, runtime namespace, reviewed namespace");
        }
        MemoryMappingTree tree = read(arguments[0]);
        int runtime = namespace(tree, arguments[1]);
        int reviewed = namespace(tree, arguments[2]);
        if (runtime == reviewed) {
            throw new IOException("runtime and reviewed mapping namespaces must differ");
        }
        try (BufferedWriter output = new BufferedWriter(
                new OutputStreamWriter(System.out, StandardCharsets.UTF_8))) {
            for (var owner : tree.getClasses()) {
                String name = required(owner.getName(reviewed), "reviewed class name");
                record(output, "C", name, required(owner.getName(runtime), "runtime class name"));
                for (var field : owner.getFields()) {
                    member(output, "F", name, field, runtime, reviewed);
                }
                for (var method : owner.getMethods()) {
                    member(output, "M", name, method, runtime, reviewed);
                }
            }
        }
    }

    private static MemoryMappingTree read(String path) throws IOException {
        MemoryMappingTree tree = new MemoryMappingTree();
        try (ZipFile archive = new ZipFile(path)) {
            ZipEntry entry = null;
            var entries = archive.entries();
            while (entries.hasMoreElements()) {
                ZipEntry candidate = entries.nextElement();
                if (candidate.getName().equals("mappings/mappings.tiny")) {
                    if (entry != null) {
                        throw new IOException("mapping jar repeats mappings/mappings.tiny");
                    }
                    entry = candidate;
                }
            }
            if (entry == null || entry.isDirectory()) {
                throw new IOException("mapping jar has no mappings/mappings.tiny file");
            }
            try (var reader = new InputStreamReader(
                    archive.getInputStream(entry), StandardCharsets.UTF_8.newDecoder()
                            .onMalformedInput(CodingErrorAction.REPORT)
                            .onUnmappableCharacter(CodingErrorAction.REPORT))) {
                MappingReader.read(reader, MappingFormat.TINY_2_FILE, new UniqueSources(tree));
            }
        }
        return tree;
    }

    private static int namespace(MappingTreeView tree, String name) throws IOException {
        int namespace = tree.getNamespaceId(name);
        if (namespace == MappingTreeView.NULL_NAMESPACE_ID) {
            throw new IOException("mapping has no namespace " + name);
        }
        return namespace;
    }

    private static void member(BufferedWriter output, String kind, String owner,
            MemberMappingView member, int runtime, int reviewed) throws IOException {
        record(output, kind, owner,
                required(member.getName(reviewed), "reviewed member name"),
                required(member.getDesc(reviewed), "reviewed member descriptor"),
                required(member.getName(runtime), "runtime member name"));
        required(member.getDesc(runtime), "runtime member descriptor");
    }

    private static String required(String value, String role) throws IOException {
        if (value == null || value.isEmpty()) {
            throw new IOException("mapping is missing " + role);
        }
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            if (Character.isWhitespace(character) || Character.isISOControl(character)) {
                throw new IOException("mapping " + role + " contains whitespace or control text");
            }
        }
        return value;
    }

    private static void record(BufferedWriter output, String... columns) throws IOException {
        output.write(String.join("\t", columns));
        output.write('\n');
    }

    /** Rejects source keys before MemoryMappingTree can merge their declarations. */
    private static final class UniqueSources extends ForwardingMappingVisitor {
        private final Set<String> classes = new HashSet<>();
        private final Set<String> fields = new HashSet<>();
        private final Set<String> methods = new HashSet<>();
        private String owner;

        UniqueSources(MemoryMappingTree tree) {
            super(tree);
        }

        @Override
        public void visitNamespaces(String source, List<String> destinations) throws IOException {
            Set<String> names = new HashSet<>(destinations);
            if (names.size() != destinations.size() || !names.add(source)) {
                throw new IOException("mapping repeats a namespace name");
            }
            super.visitNamespaces(source, destinations);
        }

        @Override
        public boolean visitClass(String name) throws IOException {
            if (!classes.add(name)) {
                throw new IOException("mapping repeats source class " + name);
            }
            owner = name;
            fields.clear();
            methods.clear();
            return super.visitClass(name);
        }

        @Override
        public boolean visitField(String name, String descriptor) throws IOException {
            unique(fields, name, descriptor, "field");
            return super.visitField(name, descriptor);
        }

        @Override
        public boolean visitMethod(String name, String descriptor) throws IOException {
            unique(methods, name, descriptor, "method");
            return super.visitMethod(name, descriptor);
        }

        private void unique(Set<String> keys, String name, String descriptor, String kind)
                throws IOException {
            required(name, "source " + kind + " name");
            required(descriptor, "source " + kind + " descriptor");
            if (!keys.add(name + "\t" + descriptor)) {
                throw new IOException("mapping repeats source " + kind + " " + owner
                        + "." + name + descriptor);
            }
        }
    }
}
