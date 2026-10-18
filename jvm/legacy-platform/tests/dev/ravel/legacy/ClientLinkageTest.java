package dev.ravel.legacy;

import dev.ravel.legacy.launch.LegacyAdapters;
import dev.ravel.legacy.launch.LegacyLoader;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Modifier;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.jar.JarFile;

/** Resolve original client and companion-library references without invoking native code. */
public final class ClientLinkageTest {
    private static final String HEADER = "kind\towner\tname\tdescriptor\tcaller\treplacement\tinvocation";

    private ClientLinkageTest() {}

    public static void main(String[] args) throws Exception {
        if (args.length < 3)
            throw new IllegalArgumentException("Expected classpath.txt client.jar linkage.tsv [linkage.tsv ...]");
        resolutionRegression();
        var urls = new ArrayList<URL>();
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (!line.isBlank()) urls.add(Path.of(line).toUri().toURL());
        }
        try (LegacyLoader loader = new LegacyLoader(urls.toArray(URL[]::new),
                ClientLinkageTest.class.getClassLoader())) {
            int classes = loadClient(loader, Path.of(args[1]));
            int references = 0;
            for (int index = 2; index < args.length; index++)
                references += resolveInventory(loader, Path.of(args[index]));
            System.out.println("RAVEL_LINKAGE classes=" + classes + " references=" + references
                    + " inventories=" + (args.length - 2) + " initialize=false");
        }
    }

    private static int loadClient(ClassLoader loader, Path path) throws Exception {
        int classes = 0;
        try (JarFile client = new JarFile(path.toFile())) {
            var entries = client.entries();
            while (entries.hasMoreElements()) {
                var entry = entries.nextElement();
                if (!entry.getName().endsWith(".class")) continue;
                String name = entry.getName().substring(0, entry.getName().length() - 6);
                if (load(name, loader).getClassLoader() != loader)
                    throw new AssertionError("Official client escaped its loader: " + name);
                classes++;
            }
        }
        if (classes == 0) throw new AssertionError("No official client classes were inspected");
        return classes;
    }

    private static int resolveInventory(ClassLoader loader, Path path) throws Exception {
        var lines = Files.readAllLines(path);
        if (lines.isEmpty() || !lines.getFirst().equals(HEADER))
            throw new AssertionError("Unexpected linkage inventory header: " + path);
        int members = 0;
        for (int index = 1; index < lines.size(); index++) {
            String line = lines.get(index);
            String[] row = line.split("\t", -1);
            try {
                if (row.length != 7 || !row[5].equals("PRESENT"))
                    throw new AssertionError("Expected a present seven-column reference");
                Class<?> caller = load(row[4], loader);
                Class<?> owner = load(row[1], loader);
                var lookup = MethodHandles.privateLookupIn(caller, MethodHandles.lookup());
                lookup.accessClass(owner);
                switch (row[0]) {
                    case "METHOD", "FIELD" -> {
                        resolveMember(lookup, owner, row[2], LegacyAdapters.map(row[3]), row[6], loader);
                        members++;
                    }
                    case "SUPERCLASS" -> {
                        if (caller.getSuperclass() != owner || Modifier.isFinal(owner.getModifiers()))
                            throw new AssertionError("Invalid replacement superclass");
                    }
                    case "CLASS", "TYPE" -> { }
                    default -> throw new AssertionError("Unknown reference kind: " + row[0]);
                }
            } catch (ReflectiveOperationException | LinkageError | IllegalArgumentException | AssertionError failure) {
                throw new AssertionError(path + ":" + (index + 1) + ": " + line, failure);
            }
        }
        if (members == 0) throw new AssertionError("No required members were resolved: " + path);
        return lines.size() - 1;
    }

    private static Class<?> load(String owner, ClassLoader loader) throws ClassNotFoundException {
        return Class.forName(LegacyAdapters.map(owner).replace('/', '.'), false, loader);
    }

    private static void resolveMember(MethodHandles.Lookup lookup, Class<?> owner, String name,
                                      String descriptor, String invocation, ClassLoader loader)
            throws ReflectiveOperationException {
        if (descriptor.startsWith("(")) {
            MethodType type = MethodType.fromMethodDescriptorString(descriptor, loader);
            switch (invocation) {
                case "INVOKESTATIC" -> lookup.findStatic(owner, name, type);
                case "INVOKEVIRTUAL", "INVOKEINTERFACE" -> {
                    if (owner.isInterface() != invocation.equals("INVOKEINTERFACE"))
                        throw new AssertionError("Invocation owner kind mismatch");
                    lookup.findVirtual(owner, name, type);
                }
                case "INVOKESPECIAL" -> {
                    if (name.equals("<init>")) lookup.findConstructor(owner, type);
                    else lookup.findSpecial(owner, name, type, lookup.lookupClass());
                }
                case "-" -> resolveUnusedMethod(lookup, owner, name, type);
                default -> throw new AssertionError("Invalid method invocation: " + invocation);
            }
        } else {
            Class<?> type = MethodType.fromMethodDescriptorString("()" + descriptor, loader).returnType();
            switch (invocation) {
                case "GETSTATIC" -> lookup.findStaticGetter(owner, name, type);
                case "PUTSTATIC" -> lookup.findStaticSetter(owner, name, type);
                case "GETFIELD" -> lookup.findGetter(owner, name, type);
                case "PUTFIELD" -> lookup.findSetter(owner, name, type);
                case "-" -> resolveUnusedField(lookup, owner, name, type);
                default -> throw new AssertionError("Invalid field invocation: " + invocation);
            }
        }
    }

    // Unused constant-pool references have no instruction opcode, but must still resolve a real member.
    private static void resolveUnusedMethod(MethodHandles.Lookup lookup, Class<?> owner, String name,
                                            MethodType type) throws ReflectiveOperationException {
        if (name.equals("<init>")) {
            lookup.findConstructor(owner, type);
            return;
        }
        try {
            lookup.findStatic(owner, name, type);
        } catch (NoSuchMethodException | IllegalAccessException notStatic) {
            lookup.findVirtual(owner, name, type);
        }
    }

    private static void resolveUnusedField(MethodHandles.Lookup lookup, Class<?> owner, String name,
                                           Class<?> type) throws ReflectiveOperationException {
        try {
            lookup.findStaticGetter(owner, name, type);
        } catch (NoSuchFieldException | IllegalAccessException notStatic) {
            lookup.findGetter(owner, name, type);
        }
    }

    private static void resolutionRegression() throws Exception {
        var lookup = MethodHandles.privateLookupIn(LinkageFixture.class, MethodHandles.lookup());
        ClassLoader loader = ClientLinkageTest.class.getClassLoader();
        resolveMember(lookup, LinkageFixture.class, "<init>", "()V", "INVOKESPECIAL", loader);
        resolveMember(lookup, LinkageFixture.class, "getMessage", "()Ljava/lang/String;", "INVOKEVIRTUAL", loader);
        resolveMember(lookup, LinkageFixture.class, "privateMember", "()I", "INVOKEVIRTUAL", loader);
        resolveMember(lookup, LinkageFixture.class, "staticMember", "()I", "INVOKESTATIC", loader);
        resolveMember(lookup, LinkageFixture.class, "field", "I", "GETFIELD", loader);
        resolveMember(lookup, LinkageFixture.class, "field", "I", "PUTFIELD", loader);
        resolveMember(lookup, LinkageFixture.class, "staticField", "I", "GETSTATIC", loader);
        resolveMember(lookup, LinkageFixture.class, "staticField", "I", "PUTSTATIC", loader);
        expectMissing(lookup, "missing", "()I", "INVOKEVIRTUAL", loader);
        expectMissing(lookup, "privateMember", "()J", "INVOKEVIRTUAL", loader);
        expectMissing(lookup, "staticMember", "()I", "INVOKEVIRTUAL", loader);
        expectMissing(lookup, "privateMember", "()I", "INVOKESTATIC", loader);
        expectMissing(lookup, "field", "J", "GETFIELD", loader);
        expectMissing(lookup, "staticField", "I", "GETFIELD", loader);
    }

    private static void expectMissing(MethodHandles.Lookup lookup, String name, String descriptor,
                                      String invocation, ClassLoader loader) throws Exception {
        try {
            resolveMember(lookup, LinkageFixture.class, name, descriptor, invocation, loader);
        } catch (NoSuchMethodException | NoSuchFieldException | IllegalAccessException expected) {
            return;
        }
        throw new AssertionError("Invalid member reference resolved: " + name + descriptor + " " + invocation);
    }

    private static final class LinkageFixture extends Exception {
        private static final long serialVersionUID = 1L;
        private int field;
        private static int staticField;

        private int privateMember() { return field; }
        private static int staticMember() { return staticField; }
    }
}
