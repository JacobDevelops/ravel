package dev.ravel.jvm.screen;

import com.mojang.serialization.Codec;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ExtraCodecs;
import net.minecraft.util.datafix.DataFixTypes;
import net.minecraft.world.level.saveddata.SavedData;
import net.minecraft.world.level.saveddata.SavedDataType;

/**
 * The one object Minecraft stores on Ravel's behalf, and the only game type
 * Ravel has to subclass.
 *
 * <p>ADG-7 puts this in the sibling artifact rather than the agent jar for a
 * reason that is not convenience: {@code SavedData} is abstract, so storing a
 * Ravel-owned one means <em>defining</em> a concrete subclass of a game type,
 * and {@code SavedDataType} additionally wants a codec for it. Reflection calls
 * methods; it neither defines a subclass nor builds a codec. So this half is
 * compiled, against generated stubs, and the control flow around it stays
 * reflective in the always-present agent jar.
 *
 * <p>The payload is one opaque byte array. Nothing here parses it, measures it,
 * or knows how many owners are inside it — {@code ravel-save} owns the aggregate
 * format and every admission decision about it. This class's entire job is to be
 * a thing Minecraft's own serialization knows how to write.
 */
public final class RavelWorldData extends SavedData {

    /** What the stored object is called inside the world's data folder. */
    private static final String NAMESPACE = "ravel";
    private static final String PATH = "mods";

    /** The key the aggregate sits under inside the encoded compound. */
    private static final String FIELD = "aggregate";

    private final byte[] aggregate;

    private RavelWorldData(byte[] aggregate) {
        this.aggregate = aggregate;
    }

    /**
     * The registration Minecraft reads and writes this object through.
     *
     * <p>Built fresh rather than cached in a static, because the type carries a
     * codec and a supplier and nothing here is hot: it is touched twice per world
     * lifecycle, once at open and once per save.
     */
    static SavedDataType type() {
        // `BASE64_STRING` is already a codec over `byte[]`, so nothing here
        // hand-rolls one. `fieldOf` puts it under a key so the encoded form is
        // the compound tag the storage writes; a bare leaf codec would encode to
        // a string tag and the storage would have nothing to write.
        Codec codec = ExtraCodecs.BASE64_STRING
                .xmap(stored -> new RavelWorldData((byte[]) stored),
                        data -> ((RavelWorldData) data).aggregate)
                .fieldOf(FIELD)
                .codec();
        return new SavedDataType(
                Identifier.fromNamespaceAndPath(NAMESPACE, PATH),
                () -> new RavelWorldData(new byte[0]),
                codec,
                DataFixTypes.SAVED_DATA_COMMAND_STORAGE);
    }

    /** Wraps replacement bytes as the object the storage will write. */
    static SavedData of(byte[] aggregate) {
        RavelWorldData held = new RavelWorldData(aggregate);
        // The pinned `SavedDataStorage.set` marks the replacement dirty exactly
        // once, so this must not do it again.
        return held;
    }

    /** The stored aggregate, or {@code null} when there is no Ravel object. */
    static byte[] bytes(Object stored) {
        if (!(stored instanceof RavelWorldData held)) {
            return null;
        }
        return held.aggregate;
    }
}
