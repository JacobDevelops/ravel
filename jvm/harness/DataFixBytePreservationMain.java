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
 * the production class Minecraft stores, through the same entry points the
 * always-present agent jar reaches it by. {@code replacement(byte[])} is the
 * whole array the agent jar hands the storage's setter, so both halves of it are
 * used as production uses them: its {@link SavedDataType} carries the codec that
 * serialises a real save, and its {@link SavedData} is the object that save
 * writes. {@code lookup()} is the array the getter is asked with, so its type
 * carries the codec a real load parses through, and {@code bytes} reads the
 * aggregate back out. So the codec that encodes here is the one a save encodes
 * with and the codec that decodes here is the one a load decodes with, not a
 * second construction of either. A measurement that hand-rolled an equivalent
 * codec would be a measurement of the copy, and would keep passing after
 * production's shape moved out from under it.
 *
 * <p>Writing and reading through two separately built registrations is only one
 * measurement if the two are the same registration, so that is required rather
 * than assumed: {@link #assertSameRegistration} refuses loudly unless they agree
 * on every component this version's {@code SavedDataType} defines identity by
 * that a measurement can compare — the same id, the same {@link DataFixTypes},
 * and codecs that encode one probe object to the same tag. Record equality
 * cannot answer this, because the class builds a fresh supplier and a fresh
 * codec per call and neither compares equal to the last one.
 *
 * <p>Reaching them this way rather than through the class's own private
 * constructors is what keeps the measurement from widening production's surface:
 * {@code type()} and {@code of(byte[])} are private because nothing outside the
 * class may build either, and this harness is outside the class.
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

    /** The aggregate the two registrations are compared over. */
    private static final byte[] REGISTRATION_PROBE = {0, 1, -1, -128, 127};

    private DataFixBytePreservationMain() {}

    public static void main(String[] arguments) {
        SharedConstants.tryDetectVersion();
        int currentVersion = SharedConstants.getCurrentVersion().dataVersion().version();
        if (currentVersion != PINNED_CURRENT_DATA_VERSION) {
            throw new AssertionError(
                    "detected data version " + currentVersion
                            + " differs from pinned version " + PINNED_CURRENT_DATA_VERSION);
        }

        SavedDataType written = replacement(REGISTRATION_PROBE).type();
        SavedDataType asked = askedType();
        assertSameRegistration(written, asked);
        Codec<SavedData> readCodec = aggregateCodec(asked);
        DataFixTypes fixType = asked.dataFixType();
        DataFixer fixer = DataFixers.getDataFixer();
        List<Payload> payloads = payloads();

        long cases = 0;
        for (int sourceVersion = OLDEST_SAVED_DATA_VERSION;
                sourceVersion <= currentVersion;
                sourceVersion++) {
            for (Payload payload : payloads) {
                assertPreserved(readCodec, fixType, fixer, sourceVersion, currentVersion, payload);
                cases++;
            }
        }

        System.out.printf(
                "datafix-byte-preservation=passed fixer=%s shape=%s range=%d..%d versions=%d "
                        + "payloads=%d cases=%d%n",
                fixType.name(),
                observedShape(payloads.getFirst()),
                OLDEST_SAVED_DATA_VERSION,
                currentVersion,
                currentVersion - OLDEST_SAVED_DATA_VERSION + 1,
                payloads.size(),
                cases);
    }

    /**
     * The registration production asks this version's storage with.
     *
     * <p>{@code lookup()} is what {@code dev.ravel.jvm.Persistence} passes to
     * the storage's getter, and under a codec-era version its sole element is
     * the {@link SavedDataType}. The declared return is {@code Object[]}, so
     * nothing here is a compile-time guarantee: this is a runtime refusal, and
     * it is the only thing standing between production changing what it asks the
     * storage with and this measurement quietly measuring something else.
     */
    private static SavedDataType askedType() {
        Object[] lookup = RavelWorldData.lookup();
        if (lookup.length != 1 || !(lookup[0] instanceof SavedDataType type)) {
            throw new AssertionError(
                    "production asks this version's storage with "
                            + Arrays.toString(lookup)
                            + ", which is not the single saved-data type this measurement decodes"
                            + " through");
        }
        return type;
    }

    /**
     * The pair production hands this version's storage as a replacement.
     *
     * <p>{@code replacement(byte[])} is passed whole to the storage's setter, so
     * its first element is the registration a real save serialises with and its
     * second is the object that save writes. Both are taken from one call, which
     * is what makes the encoding below the encoding production performs rather
     * than an encoding assembled from two of them. The object arrives already
     * marked dirty, which the fixer cannot see.
     */
    private static Registration replacement(byte[] aggregate) {
        Object[] handed = RavelWorldData.replacement(aggregate);
        if (handed.length != 2
                || !(handed[0] instanceof SavedDataType type)
                || !(handed[1] instanceof SavedData held)) {
            throw new AssertionError(
                    "production hands this version's storage "
                            + Arrays.toString(handed)
                            + ", which is not the saved-data type and object this measurement"
                            + " encodes through");
        }
        return new Registration(type, held);
    }

    /**
     * Refuses unless production writes and reads through one registration.
     *
     * <p>The two arrive from separate {@code type()} calls, so they are never the
     * same object and never {@code equals}: the record's supplier and codec
     * components are freshly built lambdas. What can be compared is what the
     * storage acts on — the id it files the object under, the {@link DataFixTypes}
     * it updates through, and the bytes the codec produces. A version that
     * registered a different codec on one of the two paths would encode the probe
     * differently, and the run stops here instead of reporting one answer for two
     * shapes.
     */
    private static void assertSameRegistration(SavedDataType written, SavedDataType asked) {
        if (written == asked) {
            return;
        }
        SavedData probe = replacement(REGISTRATION_PROBE).held();
        boolean same = written.id().equals(asked.id())
                && written.dataFixType() == asked.dataFixType()
                && encodeWith(written, probe, "the written registration's probe")
                        .equals(encodeWith(asked, probe, "the asked registration's probe"));
        if (!same) {
            throw new AssertionError(
                    "production writes through " + describe(written, probe)
                            + " but reads through " + describe(asked, probe)
                            + ", so these are two registrations and not the one this measurement"
                            + " reports on");
        }
    }

    /** One registration, as the parts a second one can be compared against. */
    private static String describe(SavedDataType type, SavedData probe) {
        return type.id()
                + " fixer=" + type.dataFixType().name()
                + " probe=" + encodeWith(type, probe, "a registration's probe");
    }

    /**
     * Narrows the production type's codec to the interface it is used through.
     *
     * <p>The type production hands the storage is a raw {@link SavedDataType},
     * because the stubs the screen jar compiles against carry erased descriptors,
     * so its {@code codec()} arrives raw too. Raw erases {@code getOrThrow}'s exception
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
    private static String observedShape(Payload sample) {
        Tag payload = encode(sample);
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
        root.put(STORAGE_FIELD, encode(payload));
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

    private static Tag encode(Payload payload) {
        Registration handed = replacement(payload.bytes());
        return encodeWith(handed.type(), handed.held(), payload.name());
    }

    private static Tag encodeWith(SavedDataType type, SavedData held, String what) {
        return aggregateCodec(type).encodeStart(NbtOps.INSTANCE, held)
                .getOrThrow(error -> new AssertionError("cannot encode " + what + ": " + error));
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

    /** What one {@code replacement(byte[])} call hands the storage. */
    private record Registration(SavedDataType type, SavedData held) {}

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
