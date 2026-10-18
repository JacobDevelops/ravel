import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassModel;
import java.lang.classfile.constantpool.ClassEntry;
import java.lang.classfile.constantpool.FieldRefEntry;
import java.lang.classfile.constantpool.MemberRefEntry;
import java.lang.classfile.constantpool.Utf8Entry;
import java.lang.classfile.constantpool.MethodHandleEntry;
import java.lang.classfile.instruction.FieldInstruction;
import java.lang.classfile.instruction.InvokeInstruction;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.jar.JarFile;
import java.util.regex.Pattern;

/** JDK 24+ source launcher: java LegacySurface.java client.jar [replacement-api.jar ...]. */
public final class LegacySurface {
    private static final Pattern DESCRIPTOR_CLASS = Pattern.compile("L(org/lwjgl/[^;]+);");
    private static final Map<String, String> ADAPTERS = adapters();
    private record Api(Set<String> members, String parent, java.util.List<String> interfaces, boolean isFinal) {}

    public static void main(String[] args) throws IOException {
        boolean adapted = args.length > 0 && args[0].equals("--adapted");
        if (adapted) args = java.util.Arrays.copyOfRange(args, 1, args.length);
        if (args.length == 0 || (adapted && args.length < 2))
            throw new IllegalArgumentException("Expected [--adapted] client.jar [replacement-api.jar ...]");
        Map<String, Api> api = new HashMap<>();
        for (int i = 1; i < args.length; i++) readApi(args[i], api, false);
        Map<String, Api> client = new HashMap<>();
        readApi(args[0], client, false);
        Map<String, Api> linked = new HashMap<>(api);
        readApi(args[0], linked, adapted);
        TreeSet<String> rows = new TreeSet<>();
        try (JarFile jar = new JarFile(args[0])) {
            for (var entry : jar.stream().filter(e -> e.getName().endsWith(".class")).toList()) {
                ClassModel model = ClassFile.of().parse(jar.getInputStream(entry).readAllBytes());
                scan(model, client, linked, api, args.length, adapted, rows);
            }
        }
        System.out.println("kind\towner\tname\tdescriptor\tcaller\treplacement\tinvocation");
        rows.forEach(System.out::println);
        if (adapted && rows.stream().anyMatch(row -> row.split("\t")[5].equals("MISSING"))) System.exit(1);
    }

    private static Map<String, String> adapters() {
        var owners = java.nio.file.Path.of(System.getProperty("ravel.legacy.owners",
                "jvm/legacy-platform/src/main/resources/dev/ravel/legacy/adapter-owners.txt"));
        try {
            Map<String, String> result = new HashMap<>();
            for (String suffix : java.nio.file.Files.readAllLines(owners)) {
                if (!suffix.isBlank())
                    result.put("org/lwjgl/" + suffix, "dev/ravel/legacy/" + suffix);
            }
            return Map.copyOf(result);
        } catch (IOException failure) {
            throw new UncheckedIOException(failure);
        }
    }

    private static Api api(ClassModel model, boolean adapted) {
        Set<String> members = new HashSet<>();
        model.fields().forEach(f -> members.add("FIELD\t" + f.fieldName().stringValue()
                + "\t" + descriptor(f.fieldType().stringValue(), adapted)));
        model.methods().forEach(m -> members.add("METHOD\t" + m.methodName().stringValue()
                + "\t" + descriptor(m.methodType().stringValue(), adapted)));
        return new Api(members, model.superclass().map(c -> owner(c.asInternalName(), adapted)).orElse(null),
                model.interfaces().stream().map(c -> owner(c.asInternalName(), adapted)).toList(),
                (model.flags().flagsMask() & ClassFile.ACC_FINAL) != 0);
    }

    private static void readApi(String path, Map<String, Api> target, boolean adapted) throws IOException {
        try (JarFile jar = new JarFile(path)) {
            for (var entry : jar.stream().filter(e -> e.getName().endsWith(".class")).toList()) {
                ClassModel model = ClassFile.of().parse(jar.getInputStream(entry).readAllBytes());
                target.put(model.thisClass().asInternalName(), api(model, adapted));
            }
        }
    }

    private static void scan(ClassModel model, Map<String, Api> client, Map<String, Api> linked,
                             Map<String, Api> replacements, int arguments, boolean adapted, Set<String> rows) {
        String caller = model.thisClass().asInternalName();
        Map<String, Set<String>> invocations = invocations(model);
        model.superclass().map(ClassEntry::asInternalName).filter(p -> p.startsWith("org/lwjgl/")).ifPresent(p -> {
            Api parent = replacements.get(owner(p, adapted));
            rows.add("SUPERCLASS\t" + p + "\t-\t-\t" + caller + "\t"
                    + status(arguments, parent != null && !parent.isFinal()) + "\t-");
        });
        for (var constant : model.constantPool()) {
            if (constant instanceof ClassEntry c && c.name().stringValue().startsWith("org/lwjgl/")) {
                String name = c.name().stringValue();
                rows.add("CLASS\t" + name + "\t-\t-\t" + caller + "\t"
                        + status(arguments, replacements.containsKey(owner(name, adapted))) + "\t-");
            } else if (constant instanceof MemberRefEntry m) {
                String name = m.owner().asInternalName();
                String kind = m instanceof FieldRefEntry ? "FIELD" : "METHOD";
                String member = kind + "\t" + m.name().stringValue() + "\t" + m.type().stringValue();
                if (!inheritedLwjgl(client, name, member, new HashSet<>())) continue;
                String adaptedMember = kind + "\t" + m.name().stringValue() + "\t" + descriptor(m.type().stringValue(), adapted);
                for (String invocation : invocations.getOrDefault(reference(m), Set.of("-"))) {
                    rows.add(kind + "\t" + name + "\t" + m.name().stringValue() + "\t" + m.type().stringValue()
                            + "\t" + caller + "\t" + status(arguments,
                            resolves(linked, owner(name, adapted), adaptedMember, new HashSet<>()))
                            + "\t" + invocation);
                }
            } else if (constant instanceof Utf8Entry text) {
                var matcher = DESCRIPTOR_CLASS.matcher(text.stringValue());
                while (matcher.find()) {
                    String name = matcher.group(1);
                    rows.add("TYPE\t" + name + "\t-\t-\t" + caller + "\t"
                            + status(arguments, replacements.containsKey(owner(name, adapted))) + "\t-");
                }
            }
        }
    }

    private static Map<String, Set<String>> invocations(ClassModel model) {
        Map<String, Set<String>> result = new HashMap<>();
        model.methods().forEach(method -> method.code().ifPresent(code -> {
            for (var element : code) {
                if (element instanceof InvokeInstruction invoke)
                    addInvocation(result, invoke.method(), invoke.opcode().name());
                else if (element instanceof FieldInstruction field)
                    addInvocation(result, field.field(), field.opcode().name());
            }
        }));
        for (var entry : model.constantPool()) {
            if (entry instanceof MethodHandleEntry handle) {
                String opcode = switch (handle.kind()) {
                    case 1 -> "GETFIELD";
                    case 2 -> "GETSTATIC";
                    case 3 -> "PUTFIELD";
                    case 4 -> "PUTSTATIC";
                    case 5 -> "INVOKEVIRTUAL";
                    case 6 -> "INVOKESTATIC";
                    case 7, 8 -> "INVOKESPECIAL";
                    case 9 -> "INVOKEINTERFACE";
                    default -> throw new IllegalArgumentException("Unknown method handle kind: " + handle.kind());
                };
                addInvocation(result, handle.reference(), opcode);
            }
        }
        return result;
    }

    private static void addInvocation(Map<String, Set<String>> target, MemberRefEntry member, String opcode) {
        target.computeIfAbsent(reference(member), ignored -> new TreeSet<>()).add(opcode);
    }

    private static String reference(MemberRefEntry member) {
        return member.owner().asInternalName() + "\t" + member.name().stringValue()
                + "\t" + member.type().stringValue();
    }

    private static boolean inheritedLwjgl(Map<String, Api> client, String owner, String member, Set<String> visited) {
        if (owner == null || !visited.add(owner)) return false;
        if (owner.startsWith("org/lwjgl/")) return true;
        Api type = client.get(owner);
        if (type == null || type.members().contains(member) || member.startsWith("METHOD\t<init>\t")) return false;
        if (inheritedLwjgl(client, type.parent(), member, visited)) return true;
        return type.interfaces().stream().anyMatch(i -> inheritedLwjgl(client, i, member, visited));
    }

    private static String status(int arguments, boolean present) {
        return arguments == 1 ? "UNCOMPARED" : present ? "PRESENT" : "MISSING";
    }

    private static String owner(String owner, boolean adapted) {
        return adapted ? ADAPTERS.getOrDefault(owner, owner) : owner;
    }

    private static String descriptor(String descriptor, boolean adapted) {
        if (!adapted) return descriptor;
        for (var adapter : ADAPTERS.entrySet())
            descriptor = descriptor.replace("L" + adapter.getKey() + ";", "L" + adapter.getValue() + ";");
        return descriptor;
    }

    private static boolean resolves(Map<String, Api> api, String owner, String member, Set<String> visited) {
        if (owner == null || !visited.add(owner)) return false;
        Api type = api.computeIfAbsent(owner, LegacySurface::jdkApi);
        if (type == null) return false;
        if (type.members().contains(member)) return true;
        if (member.startsWith("METHOD\t<init>\t")) return false;
        if (resolves(api, type.parent(), member, visited)) return true;
        return type.interfaces().stream().anyMatch(i -> resolves(api, i, member, visited));
    }

    private static Api jdkApi(String name) {
        if (!name.startsWith("java/")) return null;
        try (var bytes = ClassLoader.getPlatformClassLoader().getResourceAsStream(name + ".class")) {
            return bytes == null ? null : api(ClassFile.of().parse(bytes.readAllBytes()), false);
        } catch (IOException failure) {
            throw new UncheckedIOException(failure);
        }
    }
}
