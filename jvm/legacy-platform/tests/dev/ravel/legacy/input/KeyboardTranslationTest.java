package dev.ravel.legacy.input;

import org.lwjgl.glfw.GLFW;

/** Callback translation contracts without a native window. */
public final class KeyboardTranslationTest {
    private KeyboardTranslationTest() {}

    public static void main(String[] args) {
        layoutMapping();
        digitBindings();
        releaseCharacters();
        asynchronousCharacters();
        asynchronousCapacity();
        lateControlCharacters();
        repeatsAndControls();
        boundedQueue();
        Keyboard.destroy();
    }

    private static void layoutMapping() {
        check(Keyboard.legacy(GLFW.GLFW_KEY_Q, "a") == 30, "AZERTY A binding");
        check(Keyboard.legacy(GLFW.GLFW_KEY_W, "z") == 44, "AZERTY Z binding");
        check(Keyboard.legacy(GLFW.GLFW_KEY_PERIOD, "v") == 47, "Dvorak paste binding");
        check(Keyboard.legacy(GLFW.GLFW_KEY_A, "Q") == 16, "uppercase key name");
        check(Keyboard.legacy(GLFW.GLFW_KEY_WORLD_1, "<") == 86, "ISO physical fallback");
        check(Keyboard.legacy(GLFW.GLFW_KEY_WORLD_2, "§") == 86, "alternate ISO token");
        check(Keyboard.legacy(GLFW.GLFW_KEY_2, "é") == 3, "AZERTY hotbar fallback");
        check(Keyboard.legacy(GLFW.GLFW_KEY_1, "&") == 2, "AZERTY ampersand fallback");
        check(Keyboard.legacy(GLFW.GLFW_KEY_Q, null) == 16, "unnamed physical fallback");
        check(Keyboard.legacy(GLFW.GLFW_KEY_KP_1, "1") == 79, "keypad remains distinct");
        check(Keyboard.legacy(GLFW.GLFW_KEY_A, "Ā") == 30, "Unicode must not alias GLFW tokens");
        check(Keyboard.legacy(GLFW.GLFW_KEY_LEFT_CONTROL, null) == 29, "modifier mapping");
        check(Keyboard.legacy(-1, null) == 0, "unknown key");
    }

    private static void digitBindings() {
        Keyboard.destroy();
        check(Keyboard.legacy(GLFW.GLFW_KEY_4, "'") == 5, "AZERTY apostrophe hotbar key");
        check(Keyboard.legacy(GLFW.GLFW_KEY_6, "-") == 7, "AZERTY hyphen hotbar key");
        check(Keyboard.legacy(GLFW.GLFW_KEY_4, "5") == 6, "digit layout remapping");
        Keyboard.keyEvent(GLFW.GLFW_KEY_4, GLFW.GLFW_PRESS, "'");
        Keyboard.keyEvent(GLFW.GLFW_KEY_APOSTROPHE, GLFW.GLFW_PRESS, "'");
        check(Keyboard.isKeyDown(5) && Keyboard.isKeyDown(40), "independent apostrophe bindings");
        Keyboard.keyEvent(GLFW.GLFW_KEY_4, GLFW.GLFW_RELEASE, "'");
        check(!Keyboard.isKeyDown(5) && Keyboard.isKeyDown(40), "digit release preserves apostrophe");
        Keyboard.keyEvent(GLFW.GLFW_KEY_APOSTROPHE, GLFW.GLFW_RELEASE, "'");
        check(!Keyboard.isKeyDown(40), "apostrophe release clears its own state");
        Keyboard.keyEvent(GLFW.GLFW_KEY_6, GLFW.GLFW_PRESS, "-");
        Keyboard.keyEvent(GLFW.GLFW_KEY_MINUS, GLFW.GLFW_PRESS, "-");
        check(Keyboard.isKeyDown(7) && Keyboard.isKeyDown(12), "independent hyphen bindings");
        Keyboard.keyEvent(GLFW.GLFW_KEY_6, GLFW.GLFW_RELEASE, "-");
        check(!Keyboard.isKeyDown(7) && Keyboard.isKeyDown(12), "digit release preserves minus");
        Keyboard.keyEvent(GLFW.GLFW_KEY_MINUS, GLFW.GLFW_RELEASE, "-");
        check(!Keyboard.isKeyDown(12), "minus release clears its own state");
    }

    private static void releaseCharacters() {
        Keyboard.destroy();
        Keyboard.keyEvent(GLFW.GLFW_KEY_WORLD_1, GLFW.GLFW_PRESS, "<");
        Keyboard.characterEvent('<');
        expect(86, '<', true, false);
        Keyboard.keyEvent(GLFW.GLFW_KEY_WORLD_1, GLFW.GLFW_RELEASE, "<");
        expect(86, '\0', false, false);
        check(!Keyboard.next(), "release must not duplicate character");
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        Keyboard.characterEvent(0x1f600);
        expect(30, '\ud83d', true, false);
        expect(0, '\ude00', true, false);
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_RELEASE, "a");
        expect(30, '\0', false, false);
        check(!Keyboard.next(), "release must not replay surrogate pair");
        check(!Keyboard.isKeyDown(30), "release clears held state");
    }

    private static void asynchronousCharacters() {
        Keyboard.destroy();
        Keyboard.characterEvent('R');
        expect(0, 'R', true, false);
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_RELEASE, "a");
        Keyboard.characterEvent('a');
        expect(30, '\0', true, false);
        expect(30, '\0', false, false);
        expect(0, 'a', true, false);
        check(!Keyboard.isKeyDown(30), "delayed commit must not restore held key state");
        Keyboard.keyEvent(GLFW.GLFW_KEY_B, GLFW.GLFW_PRESS, "b");
        expect(48, '\0', true, false);
        Keyboard.characterEvent('b');
        expect(0, 'b', true, false);
        Keyboard.characterEvent('c');
        expect(0, 'c', true, false);
        check(!Keyboard.next(), "each asynchronous commit delivered once");
    }

    private static void lateControlCharacters() {
        lateControlCharacter(GLFW.GLFW_KEY_ENTER, 28, '\r');
        lateControlCharacter(GLFW.GLFW_KEY_BACKSPACE, 14, '\b');
        lateControlCharacter(GLFW.GLFW_KEY_TAB, 15, '\t');
    }

    private static void lateControlCharacter(int key, int legacy, char control) {
        Keyboard.destroy();
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_RELEASE, "a");
        expect(30, '\0', true, false);
        expect(30, '\0', false, false);
        Keyboard.keyEvent(key, GLFW.GLFW_PRESS, null);
        Keyboard.characterEvent('a');
        expect(0, 'a', true, false);
        expect(legacy, control, true, false);
        check(!Keyboard.isKeyDown(30), "late character must not restore held printable state");
        check(!Keyboard.next(), "control and delayed character delivered exactly once");
    }

    private static void asynchronousCapacity() {
        Keyboard.destroy();
        for (int index = 0; index < 49; index++) Keyboard.characterEvent('a');
        Keyboard.characterEvent(0x1f600);
        Keyboard.characterEvent('b');
        Keyboard.characterEvent('c');
        for (int index = 0; index < 49; index++) expect(0, 'a', true, false);
        expect(0, 'b', true, false);
        check(!Keyboard.next(), "async pair drops atomically and full queue drops new commits");
        for (int index = 0; index < 48; index++) Keyboard.characterEvent('a');
        Keyboard.characterEvent(0x1f600);
        for (int index = 0; index < 48; index++) expect(0, 'a', true, false);
        expect(0, '\ud83d', true, false);
        expect(0, '\ude00', true, false);
        check(!Keyboard.next(), "async pair fills final two slots atomically");
    }

    private static void repeatsAndControls() {
        Keyboard.destroy();
        Keyboard.keyEvent(GLFW.GLFW_KEY_BACKSPACE, GLFW.GLFW_PRESS, null);
        expect(14, '\b', true, false);
        Keyboard.keyEvent(GLFW.GLFW_KEY_BACKSPACE, GLFW.GLFW_RELEASE, null);
        expect(14, '\0', false, false);
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_REPEAT, "a");
        Keyboard.characterEvent('a');
        check(!Keyboard.next(), "repeat disabled by default");
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_RELEASE, "a");
        Keyboard.characterEvent('a');
        expect(30, '\0', false, false);
        expect(0, 'a', true, false);
        Keyboard.enableRepeatEvents(true);
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_REPEAT, "a");
        Keyboard.characterEvent('A');
        expect(30, 'A', true, true);
    }

    private static void boundedQueue() {
        Keyboard.destroy();
        for (int index = 0; index < 50; index++) {
            Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        }
        Keyboard.keyEvent(GLFW.GLFW_KEY_B, GLFW.GLFW_PRESS, "b");
        Keyboard.characterEvent('b');
        check(Keyboard.isKeyDown(48), "dropped event still updates held state");
        for (int index = 0; index < 50; index++) expect(30, '\0', true, false);
        check(!Keyboard.next(), "full queue drops newest event and character");
        for (int index = 0; index < 49; index++) {
            Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        }
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        Keyboard.characterEvent(0x1f600);
        for (int index = 0; index < 49; index++) expect(30, '\0', true, false);
        expect(30, '\0', true, false);
        check(!Keyboard.next(), "surrogate pair dropped atomically when only one slot remains");
        for (int index = 0; index < 48; index++) {
            Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        }
        Keyboard.keyEvent(GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS, "a");
        Keyboard.characterEvent(0x1f600);
        for (int index = 0; index < 48; index++) expect(30, '\0', true, false);
        expect(30, '\ud83d', true, false);
        expect(0, '\ude00', true, false);
        check(!Keyboard.next(), "surrogate pair fills the final two slots atomically");
    }

    private static void expect(int key, char character, boolean down, boolean repeat) {
        check(Keyboard.next(), "expected keyboard event");
        check(Keyboard.getEventKey() == key, "event key");
        check(Keyboard.getEventCharacter() == character, "event character");
        check(Keyboard.getEventKeyState() == down, "event state");
        check(Keyboard.isRepeatEvent() == repeat, "event repeat");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
