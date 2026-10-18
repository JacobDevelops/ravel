package dev.ravel.legacy.opengl;

public final class DisplayMode {
    private final int width;
    private final int height;
    private final int bits;
    private final int frequency;

    public DisplayMode(int width, int height) { this(width, height, 0, 0); }
    DisplayMode(int width, int height, int bits, int frequency) {
        this.width = width;
        this.height = height;
        this.bits = bits;
        this.frequency = frequency;
    }
    public int getWidth() { return width; }
    public int getHeight() { return height; }
    public int getBitsPerPixel() { return bits; }
    public int getFrequency() { return frequency; }
    public boolean isFullscreenCapable() { return bits != 0; }
    @Override public boolean equals(Object other) {
        return other instanceof DisplayMode mode && width == mode.width && height == mode.height
                && bits == mode.bits && frequency == mode.frequency;
    }
    @Override public int hashCode() { return java.util.Objects.hash(width, height, bits, frequency); }
    @Override public String toString() { return width + " x " + height + " x " + bits + " @" + frequency + "Hz"; }
}
