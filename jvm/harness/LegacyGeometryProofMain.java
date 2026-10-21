package dev.ravel.jvm;

/** Run in a fresh JVM for each selection; deliberately needs neither host nor LWJGL. */
public final class LegacyGeometryProofMain {
    private LegacyGeometryProofMain() { }

    public static void main(String[] arguments) {
        String selection = System.getProperty("ravel.renderer", "vanilla");
        boolean valid = selection.equals("vanilla") || selection.equals("geometry-proof");
        try {
            boolean selected = LegacyGeometryProof.selected();
            if (!valid) throw new AssertionError("Unsupported world replacement was accepted");
            if (selected != selection.equals("geometry-proof")) throw new AssertionError("Selection mismatch");
            long[] metrics = LegacyGeometryProof.metrics();
            if (metrics.length != LegacyGeometryProof.NAMES.length) throw new AssertionError("Metric schema mismatch");
            for (long metric : metrics) if (metric != 0) throw new AssertionError("Selection alone performed work");
            String[] info = LegacyGeometryProof.info();
            if (!info[0].equals(selection)
                    || !info[1].equals(selected ? "selected-not-initialized" : "not-selected")
                    || !info[2].contains("world-replacement-refused"))
                throw new AssertionError("Missing fidelity refusal or unexpected context creation");
            long[] identity = LegacyGeometryProof.identity();
            if (identity[0] != 0 || identity[1] != 0) throw new AssertionError("Unuploaded mesh identity");
            LegacyGeometryProof.flushMetrics();
            System.out.println("geometry-proof-selection-no-gl-ok " + selection);
        } catch (ExceptionInInitializerError rejected) {
            if (valid || !(rejected.getCause() instanceof IllegalArgumentException)
                    || !rejected.getCause().getMessage().contains("fidelity is unavailable"))
                throw new AssertionError("Wrong selection refusal", rejected);
            System.out.println("geometry-proof-selection-refused " + selection);
        }
    }
}
