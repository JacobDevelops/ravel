package dev.ravel.legacy;

import java.nio.ByteBuffer;
import org.lwjgl.system.MemoryUtil;

/** A real LWJGL native allocation, used in separate denied and granted JVMs. */
public final class NativeAccessProbe {
    private NativeAccessProbe() {}
    public static void main(String[] args) {
        if (NativeAccessProbe.class.getModule().isNativeAccessEnabled()) {
            throw new AssertionError("Ravel unnamed module must not receive native access");
        }
        Module core = org.lwjgl.Version.class.getModule();
        System.out.println("RAVEL_NATIVE_ACCESS module=" + core.getName()
                + " nativeAccess=" + core.isNativeAccessEnabled() + " ravelNativeAccess=false");
        ByteBuffer memory = MemoryUtil.memAlloc(16);
        try {
            memory.putLong(0, 8675309L);
            if (memory.getLong(0) != 8675309L) throw new AssertionError("Native allocation corrupted");
        } finally {
            MemoryUtil.memFree(memory);
        }
        System.out.println("RAVEL_NATIVE_ACCESS allocation=true");
    }
}
