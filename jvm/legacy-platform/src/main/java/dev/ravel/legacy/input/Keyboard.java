package dev.ravel.legacy.input;

import java.util.ArrayDeque;
import java.util.Arrays;
import org.lwjgl.glfw.GLFW;

public final class Keyboard {
    private record Event(int key, char character, boolean down, boolean repeat) {}
    private static final ArrayDeque<Event> EVENTS = new ArrayDeque<>(50);
    private static final int[] LEGACY_KEYS = new int[GLFW.GLFW_KEY_LAST + 1];
    private static Event pending;
    private static final boolean[] DOWN = new boolean[256];
    private static Event event = new Event(0, '\0', false, false);
    private static boolean created;
    private static boolean repeat;
    private static boolean suppressCharacters;
    static {
        row(2, "1234567890-=");
        row(16, "QWERTYUIOP[]");
        row(30, "ASDFGHJKL;'`");
        row(44, "ZXCVBNM,./");
        bind(1, GLFW.GLFW_KEY_ESCAPE); bind(14, GLFW.GLFW_KEY_BACKSPACE);
        bind(15, GLFW.GLFW_KEY_TAB); bind(28, GLFW.GLFW_KEY_ENTER);
        bind(29, GLFW.GLFW_KEY_LEFT_CONTROL); bind(42, GLFW.GLFW_KEY_LEFT_SHIFT);
        bind(43, GLFW.GLFW_KEY_BACKSLASH); bind(54, GLFW.GLFW_KEY_RIGHT_SHIFT);
        bind(86, GLFW.GLFW_KEY_WORLD_1); bind(86, GLFW.GLFW_KEY_WORLD_2);
        bind(55, GLFW.GLFW_KEY_KP_MULTIPLY); bind(56, GLFW.GLFW_KEY_LEFT_ALT);
        bind(57, GLFW.GLFW_KEY_SPACE); bind(58, GLFW.GLFW_KEY_CAPS_LOCK);
        for (int index = 0; index < 10; index++) bind(59 + index, GLFW.GLFW_KEY_F1 + index);
        bind(87, GLFW.GLFW_KEY_F11); bind(88, GLFW.GLFW_KEY_F12);
        for (int index = 0; index < 6; index++) bind(100 + index, GLFW.GLFW_KEY_F13 + index);
        bind(113, GLFW.GLFW_KEY_F19);
        bind(141, GLFW.GLFW_KEY_KP_EQUAL);
        bind(69, GLFW.GLFW_KEY_NUM_LOCK); bind(70, GLFW.GLFW_KEY_SCROLL_LOCK);
        bind(71, GLFW.GLFW_KEY_KP_7); bind(72, GLFW.GLFW_KEY_KP_8);
        bind(73, GLFW.GLFW_KEY_KP_9); bind(74, GLFW.GLFW_KEY_KP_SUBTRACT);
        bind(75, GLFW.GLFW_KEY_KP_4); bind(76, GLFW.GLFW_KEY_KP_5);
        bind(77, GLFW.GLFW_KEY_KP_6); bind(78, GLFW.GLFW_KEY_KP_ADD);
        bind(79, GLFW.GLFW_KEY_KP_1); bind(80, GLFW.GLFW_KEY_KP_2);
        bind(81, GLFW.GLFW_KEY_KP_3); bind(82, GLFW.GLFW_KEY_KP_0);
        bind(83, GLFW.GLFW_KEY_KP_DECIMAL); bind(156, GLFW.GLFW_KEY_KP_ENTER);
        bind(157, GLFW.GLFW_KEY_RIGHT_CONTROL); bind(181, GLFW.GLFW_KEY_KP_DIVIDE);
        bind(183, GLFW.GLFW_KEY_PRINT_SCREEN); bind(184, GLFW.GLFW_KEY_RIGHT_ALT);
        bind(197, GLFW.GLFW_KEY_PAUSE); bind(199, GLFW.GLFW_KEY_HOME);
        bind(200, GLFW.GLFW_KEY_UP); bind(201, GLFW.GLFW_KEY_PAGE_UP);
        bind(203, GLFW.GLFW_KEY_LEFT); bind(205, GLFW.GLFW_KEY_RIGHT);
        bind(207, GLFW.GLFW_KEY_END); bind(208, GLFW.GLFW_KEY_DOWN);
        bind(209, GLFW.GLFW_KEY_PAGE_DOWN); bind(210, GLFW.GLFW_KEY_INSERT);
        bind(211, GLFW.GLFW_KEY_DELETE); bind(219, GLFW.GLFW_KEY_LEFT_SUPER);
        bind(220, GLFW.GLFW_KEY_RIGHT_SUPER); bind(221, GLFW.GLFW_KEY_MENU);
    }
    private Keyboard() {}
    private static void bind(int legacy, int glfw) {
        LEGACY_KEYS[glfw] = legacy;
    }
    private static void row(int start, String keys) {
        for (int index = 0; index < keys.length(); index++) bind(start + index, keys.charAt(index));
    }
    static int legacy(int glfw, String name) {
        boolean printable = glfw >= GLFW.GLFW_KEY_SPACE && glfw <= GLFW.GLFW_KEY_WORLD_2;
        if (printable && name != null && name.length() == 1) {
            int character = Character.toUpperCase(name.charAt(0));
            boolean digitKey = glfw >= GLFW.GLFW_KEY_0 && glfw <= GLFW.GLFW_KEY_9;
            if (digitKey && (character < '0' || character > '9')) return LEGACY_KEYS[glfw];
            if (character >= GLFW.GLFW_KEY_SPACE && character <= GLFW.GLFW_KEY_GRAVE_ACCENT
                    && LEGACY_KEYS[character] != 0) return LEGACY_KEYS[character];
        }
        return glfw < 0 || glfw >= LEGACY_KEYS.length ? 0 : LEGACY_KEYS[glfw];
    }
    public static void attach(long window) {
        created = true;
        GLFW.glfwSetKeyCallback(window, (handle, key, scan, action, modifiers) ->
                keyEvent(key, action, GLFW.glfwGetKeyName(key, scan)));
        GLFW.glfwSetCharCallback(window, (handle, point) -> characterEvent(point));
    }
    static void keyEvent(int key, int action, String name) {
        int code = legacy(key, name);
        boolean down = action != GLFW.GLFW_RELEASE;
        if (code != 0) DOWN[code] = down;
        pending = null;
        suppressCharacters = action == GLFW.GLFW_REPEAT && !repeat;
        if (suppressCharacters || EVENTS.size() == 50) return;
        Event next = new Event(code, down ? controlCharacter(key) : '\0',
                down, action == GLFW.GLFW_REPEAT);
        EVENTS.add(next);
        if (down) pending = next;
    }
    private static char controlCharacter(int key) {
        return switch (key) {
            case GLFW.GLFW_KEY_BACKSPACE -> '\b';
            case GLFW.GLFW_KEY_TAB -> '\t';
            case GLFW.GLFW_KEY_ENTER, GLFW.GLFW_KEY_KP_ENTER -> '\r';
            default -> '\0';
        };
    }
    static void characterEvent(int point) {
        if (suppressCharacters) return;
        Event queued = pending != null && EVENTS.peekLast() == pending ? pending : null;
        Event pressed = queued != null && queued.character() == '\0' ? queued : null;
        pending = null;
        boolean supplementary = Character.isSupplementaryCodePoint(point);
        int available = 50 - EVENTS.size() + (pressed == null ? 0 : 1);
        if (available < (supplementary ? 2 : 1)) return;
        char first = supplementary ? Character.highSurrogate(point) : (char) point;
        boolean repeated = pressed != null && pressed.repeat();
        if (queued != null) EVENTS.removeLast();
        EVENTS.add(new Event(pressed == null ? 0 : pressed.key(), first, true, repeated));
        if (supplementary) {
            EVENTS.add(new Event(0, Character.lowSurrogate(point), true, repeated));
        }
        if (queued != null && pressed == null) EVENTS.add(queued);
    }
    public static void destroy() {
        created = false;
        repeat = false;
        suppressCharacters = false;
        EVENTS.clear();
        Arrays.fill(DOWN, false);
        event = new Event(0, '\0', false, false);
        pending = null;
    }
    public static boolean isCreated() { return created; }
    public static void enableRepeatEvents(boolean enabled) { repeat = enabled; }
    public static boolean isKeyDown(int key) { return key >= 0 && key < DOWN.length && DOWN[key]; }
    public static boolean next() {
        Event next = EVENTS.poll();
        if (next == null) return false;
        event = next;
        return true;
    }
    public static int getEventKey() { return event.key(); }
    public static char getEventCharacter() { return event.character(); }
    public static boolean getEventKeyState() { return event.down(); }
    public static boolean isRepeatEvent() { return event.repeat(); }
    public static String getKeyName(int key) {
        if (key >= 59 && key <= 68) return "F" + (key - 58);
        if (key >= 100 && key <= 105) return "F" + (key - 87);
        return switch (key) {
            case 0 -> "NONE"; case 1 -> "ESCAPE";
            case 2, 3, 4, 5, 6, 7, 8, 9, 10 -> Integer.toString(key - 1);
            case 11 -> "0"; case 12 -> "MINUS"; case 13 -> "EQUALS";
            case 14 -> "BACK"; case 15 -> "TAB";
            case 16, 17, 18, 19, 20, 21, 22, 23, 24, 25 ->
                    Character.toString("QWERTYUIOP".charAt(key - 16));
            case 26 -> "LBRACKET"; case 27 -> "RBRACKET"; case 28 -> "RETURN";
            case 29 -> "LCONTROL";
            case 30, 31, 32, 33, 34, 35, 36, 37, 38 ->
                    Character.toString("ASDFGHJKL".charAt(key - 30));
            case 39 -> "SEMICOLON"; case 40 -> "APOSTROPHE"; case 41 -> "GRAVE";
            case 42 -> "LSHIFT"; case 43 -> "BACKSLASH";
            case 44, 45, 46, 47, 48, 49, 50 ->
                    Character.toString("ZXCVBNM".charAt(key - 44));
            case 51 -> "COMMA"; case 52 -> "PERIOD"; case 53 -> "SLASH";
            case 54 -> "RSHIFT"; case 55 -> "MULTIPLY"; case 56 -> "LMENU";
            case 57 -> "SPACE"; case 58 -> "CAPITAL";
            case 69 -> "NUMLOCK"; case 70 -> "SCROLL"; case 71 -> "NUMPAD7";
            case 72 -> "NUMPAD8"; case 73 -> "NUMPAD9"; case 74 -> "SUBTRACT";
            case 75 -> "NUMPAD4"; case 76 -> "NUMPAD5"; case 77 -> "NUMPAD6";
            case 78 -> "ADD"; case 79 -> "NUMPAD1"; case 80 -> "NUMPAD2";
            case 81 -> "NUMPAD3"; case 82 -> "NUMPAD0"; case 83 -> "DECIMAL";
            case 86 -> "LESS";
            case 87 -> "F11"; case 88 -> "F12"; case 112 -> "KANA"; case 113 -> "F19";
            case 121 -> "CONVERT"; case 123 -> "NOCONVERT"; case 125 -> "YEN";
            case 141 -> "NUMPADEQUALS"; case 144 -> "CIRCUMFLEX"; case 145 -> "AT";
            case 146 -> "COLON"; case 147 -> "UNDERLINE"; case 148 -> "KANJI";
            case 149 -> "STOP"; case 150 -> "AX"; case 151 -> "UNLABELED";
            case 156 -> "NUMPADENTER"; case 157 -> "RCONTROL"; case 167 -> "SECTION";
            case 179 -> "NUMPADCOMMA"; case 181 -> "DIVIDE"; case 183 -> "SYSRQ";
            case 184 -> "RMENU"; case 196 -> "FUNCTION"; case 197 -> "PAUSE";
            case 199 -> "HOME"; case 200 -> "UP"; case 201 -> "PRIOR";
            case 203 -> "LEFT"; case 205 -> "RIGHT"; case 207 -> "END";
            case 208 -> "DOWN"; case 209 -> "NEXT"; case 210 -> "INSERT";
            case 211 -> "DELETE"; case 218 -> "CLEAR"; case 219 -> "LMETA";
            case 220 -> "RMETA"; case 221 -> "APPS"; case 222 -> "POWER";
            case 223 -> "SLEEP"; default -> null;
        };
    }
}
