package dev.ravel.spike4;

import java.util.ArrayList;
import java.util.List;

/**
 * The representative object flow every ADG-4 ownership model runs.
 *
 * <p>One flow reads four doubles of an entity-shaped object, computes one value
 * from them, and stores that value back. That is the shape of most per-tick mod
 * work: read a little state, decide, write a little state. What differs between
 * the models is who holds the state and therefore what crosses the boundary.
 */
public final class Flow {

    /** Payload that must be rejected as invalid input. */
    public static final double INVALID_INPUT = Double.NaN;
    /** Payload that makes the native side fail. */
    public static final double FAILING_INPUT = Double.NEGATIVE_INFINITY;
    /** Payload that makes the native side end the process. */
    public static final double CRASH_INPUT = Double.POSITIVE_INFINITY;

    /** Position and velocity: the state a flow reads. */
    public double x;
    public double y;
    public double dx;
    public double dy;
    /** The one value a flow writes. */
    public double out;

    /** Java owns the state; every flow ships the whole of what the flow needs. */
    static native double javaAuthoritative(double x, double y, double dx, double dy);

    /** The native side owns the state; Java ships a handle and the value that moved. */
    static native double rustAuthoritative(long handle, double step);

    /** Both sides hold a copy; Java ships only the value that moved. */
    static native double mirrored(double step);

    /** Seeds the native-held state and returns its handle. */
    static native long createState(double x, double y, double dx, double dy);

    /** Bytes the native side holds for this model, which is the duplicated state. */
    static native long duplicatedBytes(int model);

    private static double sink;

    public static void main(String[] arguments) throws Exception {
        String model = argument(arguments, "--model", "mirrored");
        String library = argument(arguments, "--library", "ravel_spike_adg4");
        int warmup = Integer.parseInt(argument(arguments, "--warmup", "200000"));
        int samples = Integer.parseInt(argument(arguments, "--samples", "20000"));
        int batch = Integer.parseInt(argument(arguments, "--batch", "64"));
        String mode = argument(arguments, "--mode", "measure");

        System.loadLibrary(library);
        Flow state = new Flow();
        state.x = 1.5;
        state.y = 2.5;
        state.dx = 0.25;
        state.dy = 0.75;
        long handle = createState(state.x, state.y, state.dx, state.dy);

        switch (mode) {
            case "crash" -> {
                run(model, state, handle, CRASH_INPUT);
                return;
            }
            case "hang" -> {
                Thread.sleep(Long.MAX_VALUE);
                return;
            }
            default -> {
                // Fall through to the measured trial.
            }
        }

        // The harness samples memory at this marker, before any measurement.
        System.out.println("ready");
        System.out.flush();

        long[] batches = new long[samples];
        measure(model, state, handle, warmup, batches, batch);

        List<String> failures = exerciseFailures(model, state, handle, library);
        report(
                model,
                batches,
                duplicatedBytes(modelNumber(model)),
                boundaryBytesPerFlow(model),
                failures);

        // Park until the harness has read this process's memory.
        if (System.in.read() < 0) {
            throw new IllegalStateException("the harness closed standard input without releasing");
        }
    }

    /**
     * Times batches of the flow, reporting batch totals undivided.
     *
     * <p>One flow costs less than a {@code nanoTime} pair in two of the three
     * models, so the harness divides after taking percentiles.
     */
    private static void measure(
            String model, Flow state, long handle, int warmup, long[] batches, int batch) {
        double local = 0.0;
        for (int index = 0; index < warmup; index++) {
            local += run(model, state, handle, 0.5);
        }
        for (int sample = 0; sample < batches.length; sample++) {
            long started = System.nanoTime();
            for (int index = 0; index < batch; index++) {
                local += run(model, state, handle, 0.5);
            }
            batches[sample] = System.nanoTime() - started;
        }
        sink += local;
    }

    /** One flow under the given ownership model. */
    private static double run(String model, Flow state, long handle, double step) {
        switch (model) {
            case "java-authoritative" -> {
                state.dx = step;
                state.out = javaAuthoritative(state.x, state.y, state.dx, state.dy);
                return state.out;
            }
            case "rust-authoritative" -> {
                return rustAuthoritative(handle, step);
            }
            default -> {
                state.dx = step;
                state.out = mirrored(state.dx);
                return state.out;
            }
        }
    }

    /** The number the native side uses for a model, so no string has to cross. */
    private static int modelNumber(String model) {
        return switch (model) {
            case "java-authoritative" -> 0;
            case "rust-authoritative" -> 1;
            default -> 2;
        };
    }

    /**
     * Bytes crossing the boundary for one flow, counted from the mechanism.
     *
     * <p>A double is eight bytes and a reference or handle is eight bytes on this
     * target. These are counts, not estimates: they are what the model moves.
     */
    private static long boundaryBytesPerFlow(String model) {
        return switch (model) {
            // Four values in, one result out: the whole state ships every flow.
            case "java-authoritative" -> 4L * 8L + 8L;
            // One handle and one value in, one result out.
            case "rust-authoritative" -> 8L + 8L + 8L;
            // One value in, one result out: the mirror holds the rest.
            default -> 8L + 8L;
        };
    }

    private static List<String> exerciseFailures(
            String model, Flow state, long handle, String library) {
        List<String> failures = new ArrayList<>();
        failures.add(failure("invalid-input", rejects(model, state, handle, INVALID_INPUT)));
        failures.add(failure("callback-failure", rejects(model, state, handle, FAILING_INPUT)));
        failures.add(failure("unavailable-runtime", missingRuntimeRejected(library)));
        return failures;
    }

    private static String rejects(String model, Flow state, long handle, double payload) {
        try {
            double observed = run(model, state, handle, payload);
            return Double.isNaN(observed)
                    ? "failed-closed|the flow returned a rejection value"
                    : "continued|the flow accepted the payload";
        } catch (RuntimeException | Error rejected) {
            return "failed-closed|" + rejected.getClass().getSimpleName();
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

    private static void report(
            String model,
            long[] batches,
            long duplicatedBytes,
            long boundaryBytes,
            List<String> failures) {
        StringBuilder json = new StringBuilder(batches.length * 6 + 512);
        json.append("{\"candidate\":\"").append(model).append('"');
        json.append(",\"javaVersion\":\"").append(System.getProperty("java.version")).append('"');
        json.append(",\"javaRuntime\":\"").append(System.getProperty("java.runtime.name")).append('"');
        json.append(",\"readyMs\":")
                .append(java.lang.management.ManagementFactory.getRuntimeMXBean().getUptime());
        json.append(",\"loadedClasses\":")
                .append(java.lang.management.ManagementFactory.getClassLoadingMXBean().getLoadedClassCount());
        json.append(",\"warmup\":").append(0);
        json.append(",\"boundaryBytesPerCall\":").append(boundaryBytes);
        json.append(",\"allocationsPerCall\":null");
        json.append(",\"allocatedBytesPerCall\":null");
        json.append(",\"gateMetrics\":{\"state.duplicated_bytes\":")
                .append(duplicatedBytes)
                .append(",\"state.boundary_bytes_per_flow\":")
                .append(boundaryBytes)
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
        return sink;
    }
}
