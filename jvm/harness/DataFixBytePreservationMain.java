import com.mojang.datafixers.DataFixer;
import com.mojang.serialization.Codec;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.util.datafix.DataFixTypes;

public final class DataFixBytePreservationMain {
    private static final int OLDEST_SAVED_DATA_VERSION = 1343;
    private static final int PINNED_CURRENT_DATA_VERSION = 4790;

    private DataFixBytePreservationMain() {}

    public static void main(String[] arguments) {
        SharedConstants.tryDetectVersion();
        int currentVersion = SharedConstants.getCurrentVersion().dataVersion().version();
        if (currentVersion != PINNED_CURRENT_DATA_VERSION) {
            throw new AssertionError(
                    "detected data version " + currentVersion
                            + " differs from pinned version " + PINNED_CURRENT_DATA_VERSION);
        }

        DataFixer fixer = DataFixers.getDataFixer();
        List<Payload> payloads = payloads();
        long cases = 0;
        for (int sourceVersion = OLDEST_SAVED_DATA_VERSION;
                sourceVersion <= currentVersion;
                sourceVersion++) {
            for (Payload payload : payloads) {
                assertPreserved(fixer, sourceVersion, currentVersion, payload);
                cases++;
            }
        }

        System.out.printf(
                "datafix-byte-preservation=passed fixer=SAVED_DATA_COMMAND_STORAGE range=%d..%d versions=%d payloads=%d cases=%d%n",
                OLDEST_SAVED_DATA_VERSION,
                currentVersion,
                currentVersion - OLDEST_SAVED_DATA_VERSION + 1,
                payloads.size(),
                cases);
    }

    private static void assertPreserved(
            DataFixer fixer, int sourceVersion, int currentVersion, Payload payload) {
        CompoundTag encoded = new CompoundTag();
        encoded.putInt("DataVersion", sourceVersion);
        Tag payloadTag = Codec.BYTE_BUFFER
                .encodeStart(NbtOps.INSTANCE, ByteBuffer.wrap(payload.bytes()))
                .getOrThrow(error -> new AssertionError("cannot encode " + payload.name() + ": " + error));
        encoded.put("data", payloadTag);

        CompoundTag fixed = DataFixTypes.SAVED_DATA_COMMAND_STORAGE.update(
                fixer, encoded, sourceVersion, currentVersion);
        Tag fixedPayload = fixed.get("data");
        if (fixedPayload == null) {
            throw new AssertionError(
                    "fixer removed payload at data version " + sourceVersion
                            + " for " + payload.name());
        }
        ByteBuffer decoded = Codec.BYTE_BUFFER
                .parse(NbtOps.INSTANCE, fixedPayload)
                .getOrThrow(error -> new AssertionError(
                        "cannot decode fixed " + payload.name()
                                + " at data version " + sourceVersion + ": " + error));
        byte[] observed = new byte[decoded.remaining()];
        decoded.duplicate().get(observed);
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
