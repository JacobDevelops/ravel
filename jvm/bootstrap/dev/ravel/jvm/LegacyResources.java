package dev.ravel.jvm;

import java.io.ByteArrayInputStream;
import java.io.FileNotFoundException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

/** Adapts the admitted native resource supply to the legacy client pack contract. */
final class LegacyResources {
    private static volatile Set<String> paths;
    private static Object installedClient;
    private static volatile Map<String, byte[]> assetBytes = Map.of();
    private static Map<String, byte[]> suppliedBytes = Map.of();

    private LegacyResources() {
    }

    static Set<String> paths() throws FileNotFoundException {
        Set<String> cached = paths;
        return cached == null ? loadPaths() : cached;
    }

    private static synchronized Set<String> loadPaths() throws FileNotFoundException {
        if (paths == null) {
            java.util.List<String> fields = Agent.fields(Hook.ravelLegacyResourcePaths());
            if (fields.size() < 2 || !fields.get(0).equals("1")) {
                throw new IllegalArgumentException("unsupported legacy resource document");
            }
            int count = Integer.parseInt(fields.get(1));
            if (count < 0 || fields.size() != count + 2) {
                throw new IllegalArgumentException("invalid legacy resource count");
            }
            Set<String> admitted = new LinkedHashSet<>();
            Map<String, byte[]> captured = new java.util.HashMap<>();
            for (int index = 0; index < count; index++) {
                String path = fields.get(index + 2);
                if ((!path.startsWith("assets/") && !path.startsWith("data/"))
                        || path.contains("..") || path.contains("\\") || !admitted.add(path)) {
                    throw new IllegalArgumentException("invalid admitted resource path: " + path);
                }
                byte[] bytes = Hook.ravelLegacyResource(path);
                if (bytes == null) {
                    throw new FileNotFoundException("admitted resource has no bytes: " + path);
                }
                captured.put(path, bytes);
            }
            // Publish paths last: every lookup sees bytes from this single owning-thread capture.
            suppliedBytes = Map.copyOf(captured);
            paths = Set.copyOf(admitted);
        }
        return paths;
    }

    static byte[] bytes(String path) throws FileNotFoundException {
        if (!paths().contains(path)) {
            throw new FileNotFoundException("resource outside admitted supply: " + path);
        }
        byte[] bytes = suppliedBytes.get(path);
        if (bytes == null) {
            throw new FileNotFoundException("admitted resource has no bytes: " + path);
        }
        return bytes;
    }

    static synchronized void install(Object client) throws ReflectiveOperationException, FileNotFoundException {
        if (installedClient == client) {
            return;
        }
        Set<String> namespaces = new LinkedHashSet<>();
        Map<String, byte[]> cached = new java.util.HashMap<>();
        for (String path : paths()) {
            if (path.startsWith("assets/")) {
                cached.put(path, bytes(path));
                int separator = path.indexOf('/', 7);
                if (separator > 7) {
                    namespaces.add(path.substring(7, separator));
                }
            }
        }
        assetBytes = Map.copyOf(cached);
        Class<?> contract = LegacyAccess.type("LEGACY_RESOURCE_PACK");
        Map<Method, String> operations = operations(contract);
        Class<?> identifier = LegacyAccess.type("LEGACY_IDENTIFIER");
        Method getNamespace = Members.method(identifier, LegacyNames.member("LEGACY_IDENTIFIER", "getNamespace"));
        Method getPath = Members.method(identifier, LegacyNames.member("LEGACY_IDENTIFIER", "getPath"));
        Set<String> frozenNamespaces = Set.copyOf(namespaces);
        Object pack = Proxy.newProxyInstance(contract.getClassLoader(), new Class<?>[] {contract},
                (proxy, method, arguments) -> invoke(proxy, operations.get(method), arguments,
                        frozenNamespaces, getNamespace, getPath));
        Object packs = LegacyAccess.field("LEGACY_CLIENT", "defaultResourcePacks", client);
        java.util.List.class.getMethod("add", Object.class).invoke(packs, pack);
        installedClient = client;
    }

    private static Map<Method, String> operations(Class<?> contract) throws ReflectiveOperationException {
        Map<Method, String> operations = new java.util.HashMap<>();
        for (String name : new String[] {"getIcon", "getMetadataSection", "getResource", "getName",
                "hasResource", "getNamespaces"}) {
            operations.put(Members.method(contract, LegacyNames.member("LEGACY_RESOURCE_PACK", name)), name);
        }
        operations.put(Object.class.getMethod("toString"), "getName");
        operations.put(Object.class.getMethod("hashCode"), "hashCode");
        operations.put(Object.class.getMethod("equals", Object.class), "equals");
        return Map.copyOf(operations);
    }

    private static Object invoke(Object proxy, String operation, Object[] arguments, Set<String> namespaces,
            Method getNamespace, Method getPath) throws ReflectiveOperationException, FileNotFoundException {
        return switch (operation) {
            case "getName" -> "Ravel admitted resources";
            case "getNamespaces" -> namespaces;
            case "getIcon", "getMetadataSection" -> null;
            case "hashCode" -> System.identityHashCode(proxy);
            case "equals" -> proxy == arguments[0];
            case "hasResource" -> assetBytes.containsKey(assetPath(arguments[0], getNamespace, getPath));
            case "getResource" -> {
                String path = assetPath(arguments[0], getNamespace, getPath);
                byte[] bytes = assetBytes.get(path);
                if (bytes == null) {
                    throw new FileNotFoundException(path);
                }
                yield new ByteArrayInputStream(bytes);
            }
            default -> throw new IllegalStateException("unknown legacy resource operation: " + operation);
        };
    }

    private static String assetPath(Object identifier, Method getNamespace, Method getPath)
            throws ReflectiveOperationException {
        return "assets/" + getNamespace.invoke(identifier) + "/" + getPath.invoke(identifier);
    }
}
