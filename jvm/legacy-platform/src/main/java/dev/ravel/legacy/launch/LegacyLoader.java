package dev.ravel.legacy.launch;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;

/** Loads original game/library bytes with selected LWJGL remaps and the pinned shutdown boundary. */
public final class LegacyLoader extends URLClassLoader {

    public LegacyLoader(URL[] urls, ClassLoader parent) {
        super(urls, parent);
    }

    @Override
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        synchronized (getClassLoadingLock(name)) {
            Class<?> type = findLoadedClass(name);
            if (type == null) {
                if (name.startsWith("java.") || name.startsWith("javax.")
                        || name.startsWith("jdk.") || name.startsWith("sun.")
                        || name.startsWith("org.lwjgl.") || name.startsWith("dev.ravel.")) {
                    type = getParent().loadClass(name);
                } else {
                    try {
                        type = findClass(name);
                    } catch (ClassNotFoundException absent) {
                        type = super.loadClass(name, false);
                    }
                }
            }
            if (resolve) resolveClass(type);
            return type;
        }
    }

    @Override
    protected Class<?> findClass(String name) throws ClassNotFoundException {
        URL resource = findResource(name.replace('.', '/') + ".class");
        if (resource == null) throw new ClassNotFoundException(name);
        try (var input = resource.openStream()) {
            byte[] bytes = rewrite(input.readAllBytes());
            if (name.equals("bib")) bytes = LegacyShutdown.transform(bytes, this);
            if (name.equals("com.mojang.text2speech.Narrator")) bytes = LegacyNarrator.transform(bytes);
            return defineClass(name, bytes, 0, bytes.length);
        } catch (IOException failure) {
            throw new ClassNotFoundException(name, failure);
        }
    }

    static byte[] rewrite(byte[] original) throws IOException {
        try (var input = new DataInputStream(new ByteArrayInputStream(original));
                var buffer = new ByteArrayOutputStream(original.length + 256);
                var output = new DataOutputStream(buffer)) {
            output.writeInt(input.readInt());
            output.writeShort(input.readUnsignedShort());
            output.writeShort(input.readUnsignedShort());
            int count = input.readUnsignedShort();
            output.writeShort(count);
            for (int index = 1; index < count; index++) {
                int tag = input.readUnsignedByte();
                output.writeByte(tag);
                if (tag == 1) {
                    output.writeUTF(LegacyAdapters.map(input.readUTF()));
                } else {
                    int size = constantSize(tag);
                    output.write(input.readNBytes(size));
                    if (tag == 5 || tag == 6) index++;
                }
            }
            input.transferTo(output);
            return buffer.toByteArray();
        }
    }


    private static int constantSize(int tag) throws IOException {
        return switch (tag) {
            case 3, 4, 9, 10, 11, 12, 17, 18 -> 4;
            case 5, 6 -> 8;
            case 7, 8, 16, 19, 20 -> 2;
            case 15 -> 3;
            default -> throw new IOException("Unknown constant pool tag " + tag);
        };
    }
}
