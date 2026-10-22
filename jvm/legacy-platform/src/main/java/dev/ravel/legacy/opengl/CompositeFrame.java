package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL15;

/** Selected-only replay of the complete world, substituting one acknowledged resident section. */
public final class CompositeFrame {
    private static int resident;
    private static int[] buffers;
    private static int[] vertices;
    private static boolean replacing;
    private static boolean omitting;
    private static boolean vanillaFallback;
    private static int draws;
    private static int submitted;

    private CompositeFrame() { }

    static byte[] capture(Runnable render, int buffer, int[] ids, int[] counts, int scene, int designated,
            int width, int height, boolean fallback) {
        if (buffers != null || width <= 0 || height <= 0 || width > 1920 || height > 1080)
            throw new IllegalStateException("Invalid composited frame capture");
        int pixels = Math.multiplyExact(width, height);
        ByteBuffer raw = ByteBuffer.allocate(32 + pixels * 24).order(ByteOrder.LITTLE_ENDIAN);
        ByteBuffer arm = ByteBuffer.allocateDirect(pixels * 8).order(ByteOrder.LITTLE_ENDIAN);
        resident = buffer;
        buffers = ids;
        vertices = counts;
        vanillaFallback = fallback;
        draws = 0;
        submitted = 0;
        try {
            render.run();
            read(render, arm, width, height);
            raw.position(32).put(arm);
            replacing = true;
            read(render, arm, width, height);
            raw.put(arm);
            replacing = false;
            omitting = true;
            read(render, arm, width, height);
            raw.put(arm);
            raw.position(0).putInt(scene).putInt(designated).putInt(width).putInt(height)
                    .putInt(draws).putInt(submitted).putInt(3)
                    .putInt(fallback ? 0 : counts[0] + counts[1] + counts[2]);
            return raw.array();
        } finally {
            replacing = false;
            omitting = false;
            buffers = null;
            vertices = null;
            resident = 0;
        }
    }

    private static void read(Runnable render, ByteBuffer arm, int width, int height) {
        render.run();
        arm.clear();
        GL11.glReadPixels(0, 0, width, height, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE,
                arm.slice(0, width * height * 4));
        GL11.glReadPixels(0, 0, width, height, GL11.GL_DEPTH_COMPONENT, GL11.GL_FLOAT,
                arm.slice(width * height * 4, width * height * 4));
    }

    static boolean draw(int mode, int first, int count) {
        if (!replacing && !omitting) return false;
        int bound = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        int offset = 0;
        for (int layer = 0; layer < 4; layer++) {
            if (buffers[layer] == bound && count > 0) {
                if (omitting) return true;
                if (vanillaFallback) return false;
                if (mode != GL11.GL_QUADS || first != 0 || count != vertices[layer])
                    throw new IllegalStateException("Resident world draw does not match published vanilla layer");
                submit(mode, count, offset, bound);
                draws++;
                submitted += count;
                return true;
            }
            offset += vertices[layer] * 28;
        }
        return false;
    }

    private static void submit(int mode, int count, int offset, int previous) {
        GL11.glPushClientAttrib(GL11.GL_CLIENT_VERTEX_ARRAY_BIT);
        try {
            GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, resident);
            GL11.glVertexPointer(3, GL11.GL_FLOAT, 28, (long) offset);
            GL11.glColorPointer(4, GL11.GL_UNSIGNED_BYTE, 28, 12L + offset);
            GL13.glClientActiveTexture(GL13.GL_TEXTURE0);
            GL11.glTexCoordPointer(2, GL11.GL_FLOAT, 28, 16L + offset);
            GL13.glClientActiveTexture(GL13.GL_TEXTURE1);
            GL11.glTexCoordPointer(2, GL11.GL_SHORT, 28, 24L + offset);
            GL11.glDrawArrays(mode, 0, count);
        } finally {
            GL11.glPopClientAttrib();
            GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, previous);
        }
    }
}
