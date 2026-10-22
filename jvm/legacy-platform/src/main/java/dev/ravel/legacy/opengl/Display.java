package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.util.concurrent.locks.LockSupport;
import dev.ravel.legacy.LWJGLException;
import org.lwjgl.glfw.Callbacks;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.glfw.GLFWErrorCallback;
import org.lwjgl.glfw.GLFWImage;
import org.lwjgl.glfw.GLFWVidMode;
import dev.ravel.legacy.input.Keyboard;
import dev.ravel.legacy.input.Mouse;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GLCapabilities;

public final class Display {
    private static long window;
    private static DisplayMode mode;
    private static int framebufferWidth;
    private static int framebufferHeight;
    private static int windowWidth;
    private static int windowHeight;
    private static String title = "Ravel legacy platform";
    private static boolean fullscreen;
    private static boolean resized;
    private static boolean resizable;
    private static long nextFrame;
    private static GLFWErrorCallback errors;
    private Display() {}
    private static ByteBuffer[] icons;

    private static void initialize() {
        if (errors != null) return;
        errors = GLFWErrorCallback.createPrint(System.err);
        errors.set();
        GLFW.glfwInitHint(GLFW.GLFW_PLATFORM, GLFW.GLFW_PLATFORM_X11);
        if (!GLFW.glfwInit()) {
            destroy();
            throw new IllegalStateException("GLFW initialization failed");
        }
    }
    public static void create() throws LWJGLException { create(new PixelFormat()); }
    public static void create(PixelFormat format) throws LWJGLException {
        try {
            initialize();
            DisplayMode requested = getDisplayMode();
            GLFW.glfwDefaultWindowHints();
            GLFW.glfwWindowHint(GLFW.GLFW_RESIZABLE, resizable ? GLFW.GLFW_TRUE : GLFW.GLFW_FALSE);
            GLFW.glfwWindowHint(GLFW.GLFW_DEPTH_BITS, format.depthBits());
            GLFW.glfwWindowHint(GLFW.GLFW_REFRESH_RATE,
                    fullscreen ? requested.getFrequency() : GLFW.GLFW_DONT_CARE);
            // Vanilla requires fixed-function compatibility; never request a core-only context.
            window = GLFW.glfwCreateWindow(requested.getWidth(), requested.getHeight(), title,
                    fullscreen ? GLFW.glfwGetPrimaryMonitor() : 0, 0);
            if (window == 0) throw new LWJGLException("Unable to create OpenGL compatibility window");
            GLFW.glfwMakeContextCurrent(window);
            GLCapabilities capabilities = GL.createCapabilities();
            if (!capabilities.OpenGL20) throw new LWJGLException("Vanilla requires OpenGL 2.0");
            GLFW.glfwSetFramebufferSizeCallback(window, (handle, width, height) -> {
                framebufferWidth = width;
                framebufferHeight = height;
                resized = true;
            });
            GLFW.glfwSetWindowSizeCallback(window, (handle, width, height) -> {
                windowWidth = width;
                windowHeight = height;
            });
            refreshSize();
            Keyboard.attach(window);
            Mouse.attach(window);
            System.out.println("RAVEL_PLATFORM context=" + GL11.glGetString(GL11.GL_VERSION));
            if (icons != null) setIcon(icons);
        } catch (LWJGLException | RuntimeException | Error failure) {
            destroy();
            throw failure;
        }
    }
    public static long window() { return window; }
    public static boolean isCreated() { return window != 0; }
    public static boolean isActive() { return GLFW.glfwGetWindowAttrib(window, GLFW.GLFW_FOCUSED) != 0; }
    public static boolean isCloseRequested() { return GLFW.glfwWindowShouldClose(window); }
    public static int getWidth() { return window == 0 ? getDisplayMode().getWidth() : framebufferWidth; }
    public static int getHeight() { return window == 0 ? getDisplayMode().getHeight() : framebufferHeight; }
    public static int windowWidth() { return windowWidth; }
    public static int windowHeight() { return windowHeight; }
    public static DisplayMode getDisplayMode() {
        if (mode == null) mode = getDesktopDisplayMode();
        return mode;
    }
    private static void refreshSize() {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            java.nio.IntBuffer width = stack.mallocInt(1);
            java.nio.IntBuffer height = stack.mallocInt(1);
            GLFW.glfwGetWindowSize(window, width, height);
            windowWidth = width.get(0);
            windowHeight = height.get(0);
            GLFW.glfwGetFramebufferSize(window, width, height);
            framebufferWidth = width.get(0);
            framebufferHeight = height.get(0);
        }
    }
    public static DisplayMode getDesktopDisplayMode() {
        initialize();
        GLFWVidMode desktop = GLFW.glfwGetVideoMode(GLFW.glfwGetPrimaryMonitor());
        if (desktop == null) throw new IllegalStateException("No primary display mode");
        return new DisplayMode(desktop.width(), desktop.height(),
                desktop.redBits() + desktop.greenBits() + desktop.blueBits(), desktop.refreshRate());
    }
    public static DisplayMode[] getAvailableDisplayModes() {
        initialize();
        GLFWVidMode.Buffer modes = GLFW.glfwGetVideoModes(GLFW.glfwGetPrimaryMonitor());
        if (modes == null) throw new IllegalStateException("No display modes");
        DisplayMode[] result = new DisplayMode[modes.remaining()];
        for (int index = 0; index < result.length; index++) {
            GLFWVidMode item = modes.get(index);
            result[index] = new DisplayMode(item.width(), item.height(),
                    item.redBits() + item.greenBits() + item.blueBits(), item.refreshRate());
        }
        return result;
    }
    public static void setDisplayMode(DisplayMode value) {
        mode = value;
        fullscreen = fullscreen && value.isFullscreenCapable();
        if (window != 0) applyMode();
    }
    private static void applyMode() {
        if (!fullscreen && GLFW.glfwGetWindowMonitor(window) == 0) {
            GLFW.glfwSetWindowSize(window, mode.getWidth(), mode.getHeight());
        } else {
            GLFW.glfwSetWindowMonitor(window, fullscreen ? GLFW.glfwGetPrimaryMonitor() : 0,
                    80, 80, mode.getWidth(), mode.getHeight(),
                    fullscreen ? mode.getFrequency() : GLFW.GLFW_DONT_CARE);
        }
        refreshSize();
    }
    public static void setFullscreen(boolean value) {
        boolean target = value && getDisplayMode().isFullscreenCapable();
        if (fullscreen == target) return;
        fullscreen = target;
        if (window != 0) applyMode();
    }
    public static boolean isFullscreen() { return fullscreen; }
    public static void setTitle(String value) {
        title = value;
        if (window != 0) GLFW.glfwSetWindowTitle(window, value);
    }
    public static void setResizable(boolean value) {
        resizable = value;
        if (window != 0) GLFW.glfwSetWindowAttrib(window, GLFW.GLFW_RESIZABLE,
                value ? GLFW.GLFW_TRUE : GLFW.GLFW_FALSE);
    }
    public static int setIcon(ByteBuffer[] icons) {
        Display.icons = icons;
        if (window == 0) return icons.length;
        try (MemoryStack stack = MemoryStack.stackPush()) {
            GLFWImage.Buffer images = GLFWImage.malloc(icons.length, stack);
            for (int index = 0; index < icons.length; index++) {
                ByteBuffer pixels = icons[index];
                int side = (int) Math.sqrt(pixels.remaining() / 4.0);
                if (!pixels.isDirect()) {
                    ByteBuffer direct = stack.malloc(pixels.remaining());
                    direct.put(0, pixels, pixels.position(), pixels.remaining());
                    pixels = direct;
                }
                images.position(index).width(side).height(side).pixels(pixels);
            }
            images.position(0);
            GLFW.glfwSetWindowIcon(window, images);
        }
        return icons.length;
    }
    public static void setVSyncEnabled(boolean enabled) {
        if (window != 0) GLFW.glfwSwapInterval(enabled ? 1 : 0);
    }
    public static void update() {
        ResidentWorldDraw.presented(getWidth(), getHeight());
        GLFW.glfwSwapBuffers(window);
        GLFW.glfwPollEvents();
    }
    public static boolean wasResized() {
        boolean value = resized;
        resized = false;
        return value;
    }
    public static void sync(int fps) {
        if (fps <= 0) return;
        long now = System.nanoTime();
        nextFrame = Math.max(now, nextFrame + 1_000_000_000L / fps);
        LockSupport.parkNanos(Math.max(0, nextFrame - now));
    }
    public static void destroy() {
        Keyboard.destroy();
        Mouse.destroy();
        resized = false;
        nextFrame = 0;
        if (window != 0) {
            Callbacks.glfwFreeCallbacks(window);
            GLFW.glfwMakeContextCurrent(0);
            GLFW.glfwDestroyWindow(window);
            window = 0;
            GL.setCapabilities(null);
        }
        GLFW.glfwTerminate();
        if (errors != null) {
            GLFW.glfwSetErrorCallback(null);
            errors.free();
            errors = null;
        }
    }
}
