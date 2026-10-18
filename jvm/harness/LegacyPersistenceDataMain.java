import java.lang.reflect.InvocationTargetException;
import java.util.Arrays;

/** Exercises the actual generated legacy sibling, not a substitute SavedData. */
public final class LegacyPersistenceDataMain {
    private LegacyPersistenceDataMain() {}

    public static void main(String[] arguments) throws ReflectiveOperationException {
        Class<?> data = Class.forName(arguments[0]);
        byte[] first = {1, 2, 3};
        Object[] initial = replacement(data, first);
        Object live = initial[1];
        first[0] = 9;
        require(Arrays.equals(bytes(data, live), new byte[] {1, 2, 3}), "stage aliases input");
        Object[] next = replacement(data, new byte[] {4, 5});
        require(Arrays.equals(bytes(data, live), new byte[] {1, 2, 3}), "stage mutates live");
        data.getMethod("commit", Object.class, Object.class).invoke(null, live, next[1]);
        require(Arrays.equals(bytes(data, live), new byte[] {4, 5}), "commit misses live object");
        reject(data, live, new byte[1024 * 1024 + 1]);
        reject(data, live, null);
        require(Arrays.equals(bytes(data, live), new byte[] {4, 5}), "failure destroys checkpoint");
        require(bytes(data, null) == null, "absent data does not remain absent");
        Object[] lookup = (Object[]) data.getMethod("lookup").invoke(null);
        require(lookup[0] == data && lookup[1].equals(initial[0]), "class/key lookup differs");
        System.out.println("legacy saved-data staging, live identity and failure preservation passed");
    }

    private static Object[] replacement(Class<?> data, byte[] bytes)
            throws ReflectiveOperationException {
        return (Object[]) data.getMethod("replacement", byte[].class).invoke(null, (Object) bytes);
    }

    private static byte[] bytes(Class<?> data, Object held) throws ReflectiveOperationException {
        return (byte[]) data.getMethod("bytes", Object.class).invoke(null, held);
    }

    private static void reject(Class<?> data, Object live, byte[] bytes)
            throws ReflectiveOperationException {
        try {
            replacement(data, bytes);
            throw new AssertionError("invalid replacement admitted for " + live.getClass());
        } catch (InvocationTargetException refused) {
            require(refused.getCause() instanceof IllegalArgumentException,
                    "invalid replacement failed for an unrelated reason");
        }
    }

    private static void require(boolean value, String message) {
        if (!value) {
            throw new AssertionError(message);
        }
    }
}
