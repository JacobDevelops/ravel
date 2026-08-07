package net.minecraft.network.chat;

public interface Component {
    static MutableComponent literal(String text) {
        return new MutableComponent(text);
    }

    static MutableComponent translatable(String key) {
        return new MutableComponent(key);
    }

    String text();
}
