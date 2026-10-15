package dev.ravel.outward;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;

/** Shared action workload; trivial arithmetic isolates boundary-crossing costs. */
public final class Action {

    /** Action identifier that makes the performed method throw. */
    public static final int FAILING_ACTION = -1;

    /** Bytes one instruction occupies: an identifier and one argument. */
    public static final int INSTRUCTION_BYTES = 4 + 8;

    /** What every performed action accumulates into, so none can be deleted. */
    private static double performed;

    /** Public so native candidates can resolve and call the game action by name. */
    public static double perform(int action, double argument) {
        if (action == FAILING_ACTION) {
            throw new IllegalStateException("the game refused the action");
        }
        performed += argument * action;
        return performed;
    }

    /** Native side calls {@link #perform} per action, resolving it each time. */
    static native double directLookup(int actions);

    /** Native side calls {@link #perform} per action through a cached identifier. */
    static native double directCached(int actions);

    /** Native side returns the instructions and this class performs them. */
    static native byte[] deferred(int actions);

    /** Native side calls the failing action and reports what it observed. */
    static native String directFailure();

    /** Native side returns one instruction naming the failing action. */
    static native byte[] deferredFailure();

    private static double sink;

    public static void main(String[] arguments) throws Exception {
        String candidate = argument(arguments, "--candidate", "deferred");
        String library = argument(arguments, "--library", "ravel_spike_outward");
        int warmup = Integer.parseInt(argument(arguments, "--warmup", "100000"));
        int samples = Integer.parseInt(argument(arguments, "--samples", "20000"));
        int batch = Integer.parseInt(argument(arguments, "--batch", "64"));
        int actions = Integer.parseInt(argument(arguments, "--actions", "8"));

        System.loadLibrary(library);

        // The harness samples memory at this marker, before any measurement.
        System.out.println("ready");
        System.out.flush();

        long[] batches = new long[samples];
        measure(candidate, warmup, batches, batch, actions);

        report(candidate, batches, actions, exerciseFailures(library));

        // Park until the harness has read this process's memory.
        if (System.in.read() < 0) {
            throw new IllegalStateException("the harness closed standard input without releasing");
        }
    }

    /** Times undivided batches; the harness divides after taking percentiles. */
    private static void measure(
            String candidate, int warmup, long[] batches, int batch, int actions) {
        double local = 0.0;
        for (int index = 0; index < warmup; index++) {
            local += request(candidate, actions);
        }
        for (int sample = 0; sample < batches.length; sample++) {
            long started = System.nanoTime();
            for (int index = 0; index < batch; index++) {
                local += request(candidate, actions);
            }
            batches[sample] = System.nanoTime() - started;
        }
        sink += local;
    }

    /** One request carrying {@code actions} game actions outward. */
    private static double request(String candidate, int actions) {
        switch (candidate) {
            case "direct-lookup" -> {
                return directLookup(actions);
            }
            case "direct-cached" -> {
                return directCached(actions);
            }
            default -> {
                return apply(deferred(actions));
            }
        }
    }

    /** Executes deferred instructions on the Java side. */
    private static double apply(byte[] instructions) {
        ByteBuffer reading = ByteBuffer.wrap(instructions);
        double last = 0.0;
        while (reading.remaining() >= INSTRUCTION_BYTES) {
            last = perform(reading.getInt(), reading.getDouble());
        }
        return last;
    }

    /** Exercises game-action exceptions and an unavailable native runtime. */
    private static List<String> exerciseFailures(String library) {
        List<String> failures = new ArrayList<>();
        failures.add(failure("direct-refusal", directRefusal()));
        failures.add(failure("deferred-refusal", deferredRefusal()));
        failures.add(failure("unavailable-runtime", missingRuntimeRejected(library)));
        return failures;
    }

    private static String directRefusal() {
        try {
            String observed = directFailure();
            return "failed-closed|the native side reported: " + observed;
        } catch (RuntimeException | Error escaped) {
            return "continued|the exception escaped the native frame as "
                    + escaped.getClass().getSimpleName();
        }
    }

    private static String deferredRefusal() {
        try {
            apply(deferredFailure());
            return "continued|the refusal was never raised";
        } catch (RuntimeException refused) {
            return "failed-closed|the adapter caught "
                    + refused.getClass().getSimpleName()
                    + " with no native frame on the stack";
        }
    }

    private static String missingRuntimeRejected(String library) {
        try {
            System.loadLibrary(library + "-absent");
            return "continued|a missing runtime loaded anyway";
        } catch (UnsatisfiedLinkError | RuntimeException rejected) {
            return "failed-closed|" + rejected.getClass().getSimpleName();
        }
    }

    private static String failure(String scenario, String outcomeAndDetail) {
        String[] parts = outcomeAndDetail.split("\\|", 2);
        return "{\"scenario\":\""
                + scenario
                + "\",\"outcome\":\""
                + parts[0]
                + "\",\"detail\":\""
                + parts[1]
                + "\"}";
    }

    /** Counts one inward crossing per request and one outward crossing per direct action. */
    private static long crossingsPerRequest(String candidate, int actions) {
        return switch (candidate) {
            case "direct-lookup", "direct-cached" -> 1L + actions;
            default -> 1L;
        };
    }

    /** Bytes crossing the boundary for one request, counted the same way. */
    private static long boundaryBytesPerRequest(String candidate, int actions) {
        return switch (candidate) {
            // An int in, then an identifier and a double out and a double back
            // per action, then one double home.
            case "direct-lookup", "direct-cached" -> 4L + (long) actions * (4L + 8L + 8L) + 8L;
            // An int in, and the whole instruction array home once.
            default -> 4L + (long) actions * INSTRUCTION_BYTES;
        };
    }

    private static void report(
            String candidate, long[] batches, int actions, List<String> failures) {
        long crossings = crossingsPerRequest(candidate, actions);
        long bytes = boundaryBytesPerRequest(candidate, actions);
        StringBuilder json = new StringBuilder(batches.length * 6 + 512);
        json.append("{\"candidate\":\"").append(candidate).append('"');
        json.append(",\"javaVersion\":\"").append(System.getProperty("java.version")).append('"');
        json.append(",\"javaRuntime\":\"").append(System.getProperty("java.runtime.name")).append('"');
        json.append(",\"readyMs\":")
                .append(java.lang.management.ManagementFactory.getRuntimeMXBean().getUptime());
        json.append(",\"loadedClasses\":")
                .append(java.lang.management.ManagementFactory.getClassLoadingMXBean().getLoadedClassCount());
        json.append(",\"warmup\":").append(0);
        json.append(",\"boundaryBytesPerCall\":").append(bytes);
        json.append(",\"allocationsPerCall\":null");
        json.append(",\"allocatedBytesPerCall\":null");
        json.append(",\"gateMetrics\":{\"outward.crossings_per_request\":")
                .append(crossings)
                .append(",\"outward.actions_per_request\":")
                .append(actions)
                .append(",\"outward.boundary_bytes_per_request\":")
                .append(bytes)
                .append('}');
        json.append(",\"failures\":[").append(String.join(",", failures)).append(']');
        json.append(",\"batchNanos\":[");
        for (int index = 0; index < batches.length; index++) {
            if (index > 0) {
                json.append(',');
            }
            json.append(batches[index]);
        }
        json.append("]}");
        System.out.println(json);
        System.out.flush();
    }

    private static String argument(String[] arguments, String name, String fallback) {
        for (int index = 0; index + 1 < arguments.length; index++) {
            if (arguments[index].equals(name)) {
                return arguments[index + 1];
            }
        }
        return fallback;
    }

    /** Keeps the measured workload observable so the JIT cannot delete it. */
    public static double sink() {
        return sink + performed;
    }
}
