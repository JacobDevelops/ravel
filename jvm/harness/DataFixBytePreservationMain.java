import com.mojang.datafixers.DataFixer;
import com.mojang.serialization.Codec;
import dev.ravel.jvm.screen.RavelWorldData;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.util.datafix.DataFixTypes;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.saveddata.SavedData;
import net.minecraft.world.level.saveddata.SavedDataType;

/**
 * Measures whether the pinned game's fixer leaves Ravel's aggregate unchanged,
 * over the shape Ravel actually stores.
 *
 * <p>The shape is not restated here. It is taken from {@code RavelWorldData},
 * the production class Minecraft stores, by using its own {@link SavedDataType}:
 * its codec builds the payload, its {@code dataFixType()} selects the fixer, and
 * its {@code bytes} reads the aggregate back out. A measurement that hand-rolled
 * an equivalent codec would be a measurement of the copy, and would keep passing
 * after production's shape moved out from under it.
 *
 * <p>The wrapping around that payload is the pinned
 * {@code net.minecraft.world.level.storage.SavedDataStorage}'s, read from the
 * pinned class files: writing puts the codec's output under {@code data} in a
 * fresh compound and then adds {@code DataVersion}; reading takes
 * {@code DataVersion} from the root, hands the <em>root</em> to
 * {@code DataFixTypes.update}, and parses {@code root.get("data")}. So the fixer
 * sees the whole compound, not the payload alone, and the nesting is part of
 * what is being measured.
 *
 * <p>Two deliberate differences from that path, neither of which the fixer can
 * see. The source data version is written directly instead of through
 * {@code NbtUtils.addCurrentDataVersion}, because the point is to enter the
 * fixer from every older version rather than from the current one. And the ops
 * are plain {@link NbtOps} rather than the storage's registry-aware wrapper,
 * because this codec resolves no registry element; a wrapper would change who
 * answers registry lookups that are never made.
 */
public final class DataFixBytePreservationMain {
    private static final int OLDEST_SAVED_DATA_VERSION = 1343;
    private static final int PINNED_CURRENT_DATA_VERSION = 4790;

    /** The key the pinned storage puts the encoded payload under. */
    private static final String STORAGE_FIELD = "data";

    private DataFixBytePreservationMain() {}

    public static void main(String[] arguments) {
        SharedConstants.tryDetectVersion();
        int currentVersion = SharedConstants.getCurrentVersion().dataVersion().version();
        if (currentVersion != PINNED_CURRENT_DATA_VERSION) {
            throw new AssertionError(
                    "detected data version " + currentVersion
                            + " differs from pinned version " + PINNED_CURRENT_DATA_VERSION);
        }

        var type = RavelWorldData.type();
        Codec<SavedData> codec = aggregateCodec(type);
        DataFixTypes fixType = type.dataFixType();
        DataFixer fixer = DataFixers.getDataFixer();
        List<Payload> payloads = payloads();

        long cases = 0;
        for (int sourceVersion = OLDEST_SAVED_DATA_VERSION;
                sourceVersion <= currentVersion;
                sourceVersion++) {
            for (Payload payload : payloads) {
                assertPreserved(codec, fixType, fixer, sourceVersion, currentVersion, payload);
                cases++;
            }
        }

        System.out.printf(
                "datafix-byte-preservation=passed fixer=%s shape=%s range=%d..%d versions=%d "
                        + "payloads=%d cases=%d%n",
                fixType.name(),
                observedShape(codec, payloads.getFirst()),
                OLDEST_SAVED_DATA_VERSION,
                currentVersion,
                currentVersion - OLDEST_SAVED_DATA_VERSION + 1,
                payloads.size(),
                cases);
    }

    /**
     * Narrows the production type's codec to the interface it is used through.
     *
     * <p>{@code RavelWorldData.type()} is a raw {@link SavedDataType}, because
     * the stubs the screen jar compiles against carry erased descriptors, so its
     * {@code codec()} arrives raw too. Raw erases {@code getOrThrow}'s exception
     * variable to {@code Throwable}, which makes every decode a checked throw.
     * The cast restores only the bound the codec was built with: it maps to and
     * from {@code RavelWorldData}, which is a {@link SavedData}.
     */
    private static Codec<SavedData> aggregateCodec(SavedDataType type) {
        return (Codec<SavedData>) (Codec) type.codec();
    }

    /**
     * Reports the shape that was measured, read off an encoded payload.
     *
     * <p>This is printed rather than asserted. Asserting a key and a tag class
     * would restate production's shape in a second place, which is the mistake
     * this rewrite exists to remove; printing what was observed lets the record
     * of the run say which shape it covered, and lets a reader see the shape
     * change when production changes it.
     */
    private static String observedShape(Codec<SavedData> codec, Payload sample) {
        Tag payload = encode(codec, sample);
        StringBuilder shape = new StringBuilder(STORAGE_FIELD);
        if (!(payload instanceof CompoundTag compound)) {
            return shape.append('.').append(payload.getClass().getSimpleName()).toString();
        }
        shape.append(".{");
        boolean first = true;
        for (String key : compound.keySet()) {
            if (!first) {
                shape.append(',');
            }
            first = false;
            shape.append(key)
                    .append(':')
                    .append(compound.get(key).getClass().getSimpleName());
        }
        return shape.append('}').toString();
    }

    private static void assertPreserved(
            Codec<SavedData> codec,
            DataFixTypes fixType,
            DataFixer fixer,
            int sourceVersion,
            int currentVersion,
            Payload payload) {
        CompoundTag root = new CompoundTag();
        root.put(STORAGE_FIELD, encode(codec, payload));
        root.putInt("DataVersion", sourceVersion);

        CompoundTag fixed = fixType.update(fixer, root, sourceVersion, currentVersion);
        Tag fixedPayload = fixed.get(STORAGE_FIELD);
        if (fixedPayload == null) {
            throw new AssertionError(
                    "fixer removed payload at data version " + sourceVersion
                            + " for " + payload.name());
        }
        Object parsed = codec.parse(NbtOps.INSTANCE, fixedPayload)
                .getOrThrow(error -> new AssertionError(
                        "cannot decode fixed " + payload.name()
                                + " at data version " + sourceVersion + ": " + error));
        byte[] observed = RavelWorldData.bytes(parsed);
        if (observed == null) {
            throw new AssertionError(
                    "the fixed payload did not decode to Ravel's own saved data at data version "
                            + sourceVersion + " for " + payload.name());
        }
        if (!Arrays.equals(payload.bytes(), observed)) {
            throw new AssertionError(
                    "fixer mutated " + payload.name()
                            + " at data version " + sourceVersion
                            + ": expectedLength=" + payload.bytes().length
                            + " observedLength=" + observed.length
                            + " expectedHash=" + Arrays.hashCode(payload.bytes())
                            + " observedHash=" + Arrays.hashCode(observed));
        }
    }

    private static Tag encode(Codec<SavedData> codec, Payload payload) {
        SavedData held = RavelWorldData.of(payload.bytes());
        return codec.encodeStart(NbtOps.INSTANCE, held)
                .getOrThrow(error -> new AssertionError(
                        "cannot encode " + payload.name() + ": " + error));
    }

    private static List<Payload> payloads() {
        List<Payload> payloads = new ArrayList<>();
        payloads.add(new Payload("empty", new byte[0]));
        payloads.add(new Payload("single-zero", new byte[] {0}));
        payloads.add(new Payload("leading-zeros", new byte[] {0, 0, 1, 0, -1, -128, 127}));
        payloads.add(new Payload("non-utf8", new byte[] {-1, -2, -128, -64, 0, -19, -96, -128}));

        byte[] fullRange = new byte[256];
        for (int index = 0; index < fullRange.length; index++) {
            fullRange[index] = (byte) index;
        }
        payloads.add(new Payload("full-byte-range", fullRange));
        payloads.add(new Payload("length-31", patterned(31)));
        payloads.add(new Payload("length-257", patterned(257)));
        payloads.add(new Payload("length-4097", patterned(4097)));
        return List.copyOf(payloads);
    }

    private static byte[] patterned(int length) {
        byte[] bytes = new byte[length];
        for (int index = 0; index < length; index++) {
            bytes[index] = (byte) ((index * 73 + 19) & 0xff);
        }
        return bytes;
    }

    private record Payload(String name, byte[] bytes) {
        private Payload {
            bytes = bytes.clone();
        }

        @Override
        public byte[] bytes() {
            return bytes.clone();
        }
    }
}
