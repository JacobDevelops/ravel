package dev.ravel.spike;

import com.sun.management.ThreadMXBean;
import java.lang.management.ManagementFactory;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.util.StringUtil;

/**
 * One measured trial of one ADG-1 candidate.
 *
 * <p>Every candidate runs this same class so the workload, warm-up, batching, and
 * reported fields are identical. The process prints one JSON report line and then
 * parks on standard input so the harness can read {@code /proc} while it is still
 * alive.
 */
public final class Trial {

    /** Payload that makes the native side abort the process. */
    public static final long CRASH_PAYLOAD = Long.MIN_VALUE + 1L;

    private static final int DEFAULT_WARMUP = 200_000;
    private static final int DEFAULT_SAMPLES = 20_000;
    private static final int DEFAULT_BATCH = 64;

    private static long sink;

    private Trial() {
    }

    public static void main(String[] arguments) throws Exception {
        String candidate = argument(arguments, "--candidate", "unknown");
        String boundary = argument(arguments, "--boundary", "none");
        String mode = argument(arguments, "--mode", "measure");
        String library = argument(arguments, "--library", "ravel_spike_adg1");
        int warmup = Integer.parseInt(argument(arguments, "--warmup", String.valueOf(DEFAULT_WARMUP)));
        int samples = Integer.parseInt(argument(arguments, "--samples", String.valueOf(DEFAULT_SAMPLES)));
        int batch = Integer.parseInt(argument(arguments, "--batch", String.valueOf(DEFAULT_BATCH)));

        if (boundary.equals("native")) {
            System.loadLibrary(library);
        }

        switch (mode) {
            case "crash" -> {
                crash(boundary);
                return;
            }
            case "hang" -> {
                // Never reports, so the harness timeout is what ends this trial.
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
        measure(warmup, batches, batch);

        // The allocation pass reuses the array the timing pass filled, so the
        // only allocation it can observe is the boundary's own.
        ThreadMXBean threads = (ThreadMXBean) ManagementFactory.getThreadMXBean();
        long allocatedBefore = threads.getCurrentThreadAllocatedBytes();
        measure(0, batches, batch);
        long allocated = threads.getCurrentThreadAllocatedBytes() - allocatedBefore;
        // A disabled counter reads -1, which must not be reported as no allocation.
        boolean allocationObserved = allocatedBefore >= 0L;

        List<String> failures = exerciseFailures(boundary, library);
        long invocations = (long) samples * batch;
        report(
                candidate,
                ManagementFactory.getRuntimeMXBean().getUptime(),
                ManagementFactory.getClassLoadingMXBean().getLoadedClassCount(),
                warmup,
                batches,
                boundary.equals("none") ? 0L : 16L,
                allocationObserved ? (double) allocated / (double) invocations : -1.0,
                failures);

        // Park until the harness has read this process's memory.
        if (System.in.read() < 0) {
            throw new IllegalStateException("the harness closed standard input without releasing");
        }
    }

    /**
     * Times batches of the pinned target method into the given array.
     *
     * <p>A single boundary transition is cheaper than one {@code nanoTime} pair, so
     * each sample times a batch and reports the total undivided. Dividing here
     * would truncate a two nanosecond call to zero; the harness divides once,
     * after taking percentiles.
     */
    private static void measure(int warmup, long[] batches, int batch) {
        long local = 0L;
        for (int index = 0; index < warmup; index++) {
            local += StringUtil.isAllowedChatCharacter(index) ? 1L : 0L;
        }

        for (int sample = 0; sample < batches.length; sample++) {
            long started = System.nanoTime();
            for (int index = 0; index < batch; index++) {
                local += StringUtil.isAllowedChatCharacter(index) ? 1L : 0L;
            }
            batches[sample] = System.nanoTime() - started;
        }
        sink += local;
    }

    private static List<String> exerciseFailures(String boundary, String library) {
        List<String> failures = new ArrayList<>();
        failures.add(failure("invalid-input", callbackRejects(boundary, Hook.INVALID_PAYLOAD)));
        failures.add(failure("callback-failure", callbackRejects(boundary, Hook.FAILING_PAYLOAD)));
        failures.add(failure("unavailable-runtime", missingRuntimeRejected(library)));
        return failures;
    }

    private static String callbackRejects(String boundary, long payload) {
        try {
            long observed =
                    boundary.equals("none") || boundary.equals("java")
                            ? Hook.onJavaCallback(payload)
                            : Hook.onNativeCallback(payload);
            return observed < 0L
                    ? "failed-closed|the callback returned a rejection sentinel"
                    : "continued|the callback accepted the payload";
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

    private static void crash(String boundary) {
        if (boundary.equals("none") || boundary.equals("java")) {
            Runtime.getRuntime().halt(134);
            return;
        }
        Hook.onNativeCallback(CRASH_PAYLOAD);
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
            String candidate,
            long readyMs,
            long loadedClasses,
            int warmup,
            long[] batches,
            long boundaryBytesPerCall,
            double allocatedBytesPerCall,
            List<String> failures) {
        StringBuilder json = new StringBuilder(batches.length * 6 + 512);
        json.append("{\"candidate\":\"").append(candidate).append('"');
        // A candidate that creates its own VM picks its own runtime, so runtime
        // identity is reported from inside the trial rather than assumed.
        json.append(",\"javaVersion\":\"").append(System.getProperty("java.version")).append('"');
        json.append(",\"javaRuntime\":\"").append(System.getProperty("java.runtime.name")).append('"');
        json.append(",\"readyMs\":").append(readyMs);
        json.append(",\"loadedClasses\":").append(loadedClasses);
        json.append(",\"transformed\":").append(Agent.transformed());
        json.append(",\"warmup\":").append(warmup);
        json.append(",\"boundaryBytesPerCall\":").append(boundaryBytesPerCall);
        json.append(",\"allocationsPerCall\":null");
        json.append(",\"allocatedBytesPerCall\":")
                .append(allocatedBytesPerCall < 0.0 ? "null" : Double.toString(allocatedBytesPerCall));
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
    public static long sink() {
        return sink;
    }
}
