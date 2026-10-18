package dev.ravel.legacy.input;

import org.lwjgl.glfw.GLFW;

/** Exercises callback translation and buffering without creating a native window. */
public final class MouseTranslationTest {
    private MouseTranslationTest() {}

    public static void main(String[] args) {
        motionAndCoordinates();
        buttonsAndWheel();
        boundedQueue();
        Mouse.destroy();
    }

    private static void motionAndCoordinates() {
        Mouse.destroy();
        Mouse.cursorEvent(10.25, 20.75, 2, 2, 100);
        expect(-1, false, 0, 20, 58);
        check(Mouse.getDX() == 20 && Mouse.getDY() == -41, "scaled relative motion");
        check(Mouse.getDX() == 0 && Mouse.getDY() == 0, "fractional remainder retained");
        Mouse.cursorEvent(10.5, 21, 2, 2, 100);
        expect(-1, false, 0, 21, 57);
        check(Mouse.getDX() == 1 && Mouse.getDY() == -1, "fractional motion accumulated");
        Mouse.cursorEvent(10, 20, 2, 2, 100);
        expect(-1, false, 0, 20, 59);
        check(Mouse.getDX() == -1 && Mouse.getDY() == 2, "reverse motion");
    }

    private static void buttonsAndWheel() {
        Mouse.destroy();
        Mouse.buttonEvent(GLFW.GLFW_MOUSE_BUTTON_RIGHT, GLFW.GLFW_PRESS, 12, 34);
        check(Mouse.isButtonDown(1), "right button held");
        expect(1, true, 0, 12, 34);
        Mouse.buttonEvent(GLFW.GLFW_MOUSE_BUTTON_RIGHT, GLFW.GLFW_RELEASE, 13, 35);
        check(!Mouse.isButtonDown(1), "right button released");
        expect(1, false, 0, 13, 35);
        Mouse.buttonEvent(-1, GLFW.GLFW_PRESS, 0, 0);
        Mouse.buttonEvent(16, GLFW.GLFW_PRESS, 0, 0);
        check(!Mouse.next(), "invalid buttons ignored");
        Mouse.scrollEvent(1, 14, 36);
        expect(-1, false, 120, 14, 36);
        Mouse.scrollEvent(-2.5, 15, 37);
        expect(-1, false, -300, 15, 37);
        check(Mouse.getScrollEventCount() == 2, "wheel callback count survives draining");
    }

    private static void boundedQueue() {
        Mouse.destroy();
        for (int index = 0; index < 50; index++) Mouse.scrollEvent(1, index, index + 1);
        Mouse.buttonEvent(0, GLFW.GLFW_PRESS, 99, 99);
        Mouse.scrollEvent(-1, 99, 99);
        check(Mouse.isButtonDown(0), "dropped button still updates held state");
        check(Mouse.getScrollEventCount() == 51, "dropped scroll remains observable");
        for (int index = 0; index < 50; index++) expect(-1, false, 120, index, index + 1);
        check(!Mouse.next(), "full queue drops newest events");
        Mouse.scrollEvent(-1, 4, 5);
        expect(-1, false, -120, 4, 5);
        Mouse.destroy();
        check(!Mouse.next() && !Mouse.isButtonDown(0), "destroy clears input state");
        check(Mouse.getScrollEventCount() == 0, "destroy resets callback counter");
    }

    private static void expect(int button, boolean down, int wheel, int x, int y) {
        check(Mouse.next(), "expected mouse event");
        check(Mouse.getEventButton() == button, "event button");
        check(Mouse.getEventButtonState() == down, "event button state");
        check(Mouse.getEventDWheel() == wheel, "event wheel");
        check(Mouse.getEventX() == x && Mouse.getEventY() == y, "event coordinates");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
