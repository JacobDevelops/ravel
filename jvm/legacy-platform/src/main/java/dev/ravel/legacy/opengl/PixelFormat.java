package dev.ravel.legacy.opengl;

public final class PixelFormat {
    private final int depthBits;
    public PixelFormat() { this(8); }
    private PixelFormat(int depthBits) { this.depthBits = depthBits; }
    public PixelFormat withDepthBits(int bits) { return new PixelFormat(bits); }
    int depthBits() { return depthBits; }
}
