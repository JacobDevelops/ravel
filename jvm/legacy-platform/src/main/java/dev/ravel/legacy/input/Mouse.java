package dev.ravel.legacy.input;

import java.util.ArrayDeque;
import java.util.Arrays;
import org.lwjgl.glfw.GLFW;
import dev.ravel.legacy.opengl.Display;
import org.lwjgl.system.MemoryStack;

public final class Mouse {
    private record Event(int button, boolean down, int wheel, int x, int y) {}
    private static final ArrayDeque<Event> EVENTS = new ArrayDeque<>(50);
    private static final boolean[] BUTTONS = new boolean[16];
    private static Event event = new Event(-1, false, 0, 0, 0);
    private static long window;
    private static double x;
    private static double y;
    private static double dx;
    private static double dy;
    private static long scrollEventCount;
    private static boolean inside;
    private static boolean grabbed;
    private static boolean restorePosition;
    private static int restoreX;
    private static int restoreY;
    private Mouse() {}

    public static void attach(long handle) {
        window = handle;
        primePosition();
        inside = GLFW.glfwGetWindowAttrib(handle, GLFW.GLFW_HOVERED) != 0;
        GLFW.glfwSetCursorPosCallback(handle, (ignored, horizontal, vertical) ->
                cursorEvent(horizontal, vertical, scaleX(), scaleY(), Display.getHeight()));
        GLFW.glfwSetCursorEnterCallback(handle, (ignored, entered) -> inside = entered);
        GLFW.glfwSetMouseButtonCallback(handle, (ignored, button, action, modifiers) ->
                buttonEvent(button, action, getX(), getY()));
        GLFW.glfwSetScrollCallback(handle, (ignored, horizontal, vertical) ->
                scrollEvent(vertical, getX(), getY()));
    }
    static void cursorEvent(double horizontal, double vertical,
            double horizontalScale, double verticalScale, int height) {
        dx += (horizontal - x) * horizontalScale;
        dy -= (vertical - y) * verticalScale;
        x = horizontal;
        y = vertical;
        enqueue(-1, false, 0, (int) (x * horizontalScale),
                height - (int) (y * verticalScale) - 1);
    }
    static void buttonEvent(int button, int action, int horizontal, int vertical) {
        if (button < 0 || button >= BUTTONS.length) return;
        BUTTONS[button] = action != GLFW.GLFW_RELEASE;
        enqueue(button, BUTTONS[button], 0, horizontal, vertical);
    }
    static void scrollEvent(double amount, int horizontal, int vertical) {
        scrollEventCount++;
        enqueue(-1, false, (int) (amount * 120), horizontal, vertical);
    }
    private static void enqueue(int button, boolean down, int wheel, int horizontal, int vertical) {
        if (EVENTS.size() < 50) EVENTS.add(new Event(button, down, wheel, horizontal, vertical));
    }
    /** Number of delivered scroll callbacks, including callbacks dropped by a full event queue. */
    public static long getScrollEventCount() { return scrollEventCount; }
    private static double scaleX() {
        return Display.getWidth() / (double) Math.max(1, Display.windowWidth());
    }
    private static double scaleY() {
        return Display.getHeight() / (double) Math.max(1, Display.windowHeight());
    }
    private static void primePosition() {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            java.nio.DoubleBuffer horizontal = stack.mallocDouble(1);
            java.nio.DoubleBuffer vertical = stack.mallocDouble(1);
            GLFW.glfwGetCursorPos(window, horizontal, vertical);
            x = horizontal.get(0);
            y = vertical.get(0);
        }
    }
    public static boolean isCreated() { return window != 0; }
    public static boolean isInsideWindow() { return inside; }
    public static boolean isButtonDown(int button) {
        return button >= 0 && button < BUTTONS.length && BUTTONS[button];
    }
    public static int getX() { return (int) (x * scaleX()); }
    public static int getY() { return Display.getHeight() - (int) (y * scaleY()) - 1; }
    public static int getDX() { int result = (int) dx; dx -= result; return result; }
    public static int getDY() { int result = (int) dy; dy -= result; return result; }
    public static void setCursorPosition(int horizontal, int vertical) {
        restoreX = horizontal;
        restoreY = vertical;
        restorePosition = grabbed;
        GLFW.glfwSetCursorPos(window, horizontal / scaleX(),
                (Display.getHeight() - vertical - 1) / scaleY());
        primePosition();
        dx = 0;
        dy = 0;
    }
    public static void setGrabbed(boolean value) {
        if (grabbed == value) return;
        GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR,
                value ? GLFW.GLFW_CURSOR_DISABLED : GLFW.GLFW_CURSOR_NORMAL);
        grabbed = value;
        if (!value && restorePosition) setCursorPosition(restoreX, restoreY);
        restorePosition = false;
        primePosition();
        dx = 0;
        dy = 0;
    }
    public static void destroy() {
        window = 0;
        EVENTS.clear();
        Arrays.fill(BUTTONS, false);
        event = new Event(-1, false, 0, 0, 0);
        x = y = dx = dy = 0;
        scrollEventCount = 0;
        inside = grabbed = restorePosition = false;
        restoreX = restoreY = 0;
    }
    public static boolean next() {
        Event next = EVENTS.poll();
        if (next == null) return false;
        event = next;
        return true;
    }
    public static int getEventButton() { return event.button(); }
    public static boolean getEventButtonState() { return event.down(); }
    public static int getEventDWheel() { return event.wheel(); }
    public static int getEventX() { return event.x(); }
    public static int getEventY() { return event.y(); }
}
