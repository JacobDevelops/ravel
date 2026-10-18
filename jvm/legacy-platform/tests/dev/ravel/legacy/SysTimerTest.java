package dev.ravel.legacy;

/** Timer units and elapsed-time contract without an audio device or native window. */
public final class SysTimerTest {
    private SysTimerTest() {}

    public static void main(String[] args) throws InterruptedException {
        if (Sys.getTimerResolution() != 1_000L) {
            throw new AssertionError("legacy timer resolution must be milliseconds");
        }
        long before = System.nanoTime();
        long start = Sys.getTime();
        Thread.sleep(5);
        long end = Sys.getTime();
        long after = System.nanoTime();
        long elapsed = end - start;
        long upperBound = (after - before) / 1_000_000L + 1;
        if (elapsed < 1 || elapsed > upperBound) {
            throw new AssertionError("timer must advance in milliseconds: " + elapsed);
        }
    }
}
