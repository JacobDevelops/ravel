package dev.ravel.jvm;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicReference;

/** Official-game transport acceptance plus explicit adapter-boundary rejection checks. */
public final class LegacyPacketLive {
    private static final String PING = "networking:ping";
    private static final String OVERLAY = "client_only:overlay";
    private static final long TIMEOUT_NANOS = 90_000_000_000L;
    private static long clientStarted;
    private static long serverStarted;
    private static long clientBefore;
    private static long serverBefore;
    private static long clientSelections;
    private static long serverSelections;
    private static long clientHooks;
    private static long serverHooks;
    private static boolean sent;
    private static boolean clientDone;
    private static boolean serverDone;
    private static final AtomicReference<Throwable> serverFailure = new AtomicReference<>();

    private LegacyPacketLive() {}

    // Exported ONLY by the composed test host; original test mods increment after decode.
    public static native long receipts(boolean toServer);

    /** Called repeatedly on the actual client thread; supports local and dedicated servers. */
    public static boolean exercise(Object client) throws Exception {
        if (clientDone) return true;
        check(Boolean.TRUE.equals(LegacyAccess.call("LEGACY_EVENT_LOOP", "isOnSameThread", client)),
                "client driver not on owning thread");
        if (clientStarted == 0) {
            clientStarted = System.nanoTime();
            clientBefore = receipts(false);
            clientSelections = LegacyPayload.selections(false);
            clientHooks = LegacyRuntime.hookCount("legacy.payload-client");
        }
        deadline(clientStarted, "client packet handshake");
        if (serverFailure.get() != null) throw new AssertionError("integrated packet probe", serverFailure.get());
        Object handler = LegacyAccess.call("LEGACY_CLIENT", "getNetworkHandler", client);
        if (handler == null) return false;
        Object integrated = LegacyAccess.call("LEGACY_CLIENT", "getServer", client);
        if (integrated != null) schedule(integrated, () -> {
            try { exerciseServer(integrated); }
            catch (Throwable failure) { serverFailure.compareAndSet(null, failure); }
        });
        if (!Files.exists(marker("server-ready"))) return false;
        if (!sent) {
            LegacyAccess.call("LEGACY_CLIENT_HANDLER", "sendPacket", handler,
                    packet(PING, ping(), true, false));
            sent = true;
        }
        if (!Files.exists(marker("server-accepted")) || receipts(false) == clientBefore) return false;
        check(receipts(false) == clientBefore + 1, "actual client-bound decode not exactly once");
        check(LegacyPayload.selections(false) == clientSelections + 1,
                "client network packet selected more/less than once");
        check(LegacyRuntime.hookCount("legacy.payload-client") > clientHooks,
                "actual client packet bypassed installed target prologue");
        boundary(handler, false);
        Files.writeString(marker("client-accepted"), "actual-network-and-boundary-checked\n");
        clientDone = true;
        return true;
    }

    /** Called repeatedly by an owning server tick, also in a dedicated server process. */
    public static boolean exerciseServer(Object server) throws Exception {
        if (serverDone) return true;
        check(Boolean.TRUE.equals(LegacyAccess.call("LEGACY_EVENT_LOOP", "isOnSameThread", server)),
                "server driver not on owning thread");
        if (serverStarted == 0) {
            serverStarted = System.nanoTime();
            serverBefore = receipts(true);
            serverSelections = LegacyPayload.selections(true);
            serverHooks = LegacyRuntime.hookCount("legacy.payload-server");
        }
        deadline(serverStarted, "server packet handshake");
        Object manager = LegacyAccess.call("LEGACY_SERVER", "getPlayerManager", server);
        List<?> players = (List<?>) LegacyAccess.call("LEGACY_PLAYER_MANAGER", "getAll", manager);
        if (players.isEmpty()) return false;
        check(players.size() == 1, "packet harness requires exactly one connected fixture player");
        Object handler = LegacyAccess.field("LEGACY_SERVER_PLAYER", "networkHandler", players.getFirst());
        if (!Files.exists(marker("server-ready"))) Files.writeString(marker("server-ready"), "ready\n");
        if (receipts(true) == serverBefore) return false;
        check(receipts(true) == serverBefore + 1, "actual server-bound decode not exactly once");
        check(LegacyPayload.selections(true) == serverSelections + 1,
                "server network packet selected more/less than once");
        check(LegacyRuntime.hookCount("legacy.payload-server") > serverHooks,
                "actual server packet bypassed installed target prologue");
        boundary(handler, true);
        LegacyAccess.call("LEGACY_SERVER_HANDLER", "sendPacket", handler,
                packet(OVERLAY, overlay(), false, false));
        Files.writeString(marker("server-accepted"), "actual-network-and-boundary-checked\n");
        serverDone = true;
        return true;
    }

    private static void boundary(Object handler, boolean toServer) throws Exception {
        String channel = toServer ? PING : OVERLAY;
        byte[] valid = toServer ? ping() : overlay();
        Object owned = packet(channel, valid, toServer, true);
        Object buffer = LegacyAccess.call(packetRole(toServer), "getData", owned);
        long selected = LegacyPayload.selections(toServer);
        long accepted = receipts(toServer);
        AtomicReference<Throwable> failure = new AtomicReference<>();
        Thread offThread = new Thread(() -> {
            try { LegacyPayload.receive(handler, owned, toServer); }
            catch (Throwable problem) { failure.set(problem); }
        }, "legacy-packet-off-owner-fixture");
        offThread.start();
        offThread.join();
        if (failure.get() != null) throw new AssertionError("off-thread selection", failure.get());
        check(LegacyPayload.selections(toServer) == selected && receipts(toServer) == accepted,
                "off-thread invocation dispatched native packet");
        check(index(buffer) == 1, "off-thread inspection consumed reader index");
        LegacyPayload.receive(handler, owned, toServer);
        check(LegacyPayload.selections(toServer) == selected + 1 && receipts(toServer) == accepted + 1,
                "owning-thread packet was not decoded exactly once");
        check(index(buffer) == 1, "absolute payload copy consumed nonzero reader index");
        release(buffer);

        // Oversize is deliberately adapter-boundary-only: vanilla rejects it on the wire.
        reject(handler, packet("foreign:missing", valid, toServer, true), toServer, false);
        reject(handler, packet(channel, new byte[LegacyPayload.MAX_BYTES + 1], toServer, true), toServer, false);
        reject(handler, packet(channel, new byte[] {0, 0, 0, 100}, toServer, true), toServer, true);
        reject(handler, packet(channel, Arrays.copyOf(valid, valid.length + 1), toServer, true), toServer, true);
        byte[] wrongVersion = valid.clone();
        int version = 4 + channel.getBytes(StandardCharsets.UTF_8).length;
        wrongVersion[version + 3] ^= 0x40;
        reject(handler, packet(channel, wrongVersion, toServer, true), toServer, true);
        Object surviving = packet(channel, valid, toServer, true);
        Object survivingBuffer = LegacyAccess.call(packetRole(toServer), "getData", surviving);
        LegacyPayload.receive(handler, surviving, toServer);
        check(receipts(toServer) == accepted + 2, "rejected packet halted the original fixture mod");
        check(index(survivingBuffer) == 1, "survival packet consumed reader index");
        release(survivingBuffer);
    }

    private static void reject(Object handler, Object packet, boolean toServer, boolean selects) throws Exception {
        long accepted = receipts(toServer);
        long selected = LegacyPayload.selections(toServer);
        Object buffer = LegacyAccess.call(packetRole(toServer), "getData", packet);
        try {
            LegacyPayload.receive(handler, packet, toServer);
            check(receipts(toServer) == accepted, "rejected packet reached decoded callback");
            check(LegacyPayload.selections(toServer) == selected + (selects ? 1 : 0),
                    "wrong bounded/foreign selection decision");
            check(index(buffer) == 1, "refused payload consumed reader index");
        } finally { release(buffer); }
    }

    private static Object packet(String channel, byte[] payload, boolean toServer, boolean prefix) throws Exception {
        Class<?> byteBuf = Members.game("io.netty.buffer.ByteBuf");
        Object buffer = LegacyAccess.create("LEGACY_BUFFER",
                Members.game("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null));
        byte[] rest = payload;
        Object packet;
        if (toServer) {
            byte[] name = channel.getBytes(StandardCharsets.UTF_8);
            check(name.length < 0x80, "fixture channel needs a one-byte VarInt length");
            byteBuf.getMethod("writeByte", int.class).invoke(buffer, name.length);
            byteBuf.getMethod("writeBytes", byte[].class).invoke(buffer, (Object) name);
            byteBuf.getMethod("writeByte", int.class).invoke(buffer, prefix ? 0x5a : payload[0]);
            if (!prefix) rest = Arrays.copyOfRange(payload, 1, payload.length);
            packet = LegacyAccess.create("LEGACY_PACKET_SERVER");
            LegacyAccess.call("LEGACY_PACKET", "read", packet, buffer);
            buffer = LegacyAccess.call("LEGACY_PACKET_SERVER", "getData", packet);
        } else {
            packet = LegacyAccess.create("LEGACY_PACKET_CLIENT", channel, buffer);
            if (prefix) byteBuf.getMethod("writeByte", int.class).invoke(buffer, 0x5a);
        }
        // Official decoders and constructors enforce the wire bound; the oversize fixture expands
        // the packet's actual buffer afterwards and never calls sendPacket.
        byteBuf.getMethod("writeBytes", byte[].class).invoke(buffer, (Object) rest);
        if (prefix) byteBuf.getMethod("readerIndex", int.class).invoke(buffer, 1);
        return packet;
    }

    private static int index(Object buffer) throws Exception {
        return (Integer) LegacyAccess.call("LEGACY_BUFFER", "readerIndex", buffer);
    }

    private static void release(Object buffer) throws Exception {
        Members.game("io.netty.buffer.ByteBuf").getMethod("release").invoke(buffer);
    }

    private static String packetRole(boolean toServer) {
        return toServer ? "LEGACY_PACKET_SERVER" : "LEGACY_PACKET_CLIENT";
    }

    private static byte[] ping() throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DataOutputStream out = new DataOutputStream(bytes)) {
            text(out, PING);
            out.writeInt(1);
            out.writeInt(12022026);
            text(out, "ravel-live-packet");
            out.writeByte(1);
        }
        return bytes.toByteArray();
    }

    private static byte[] overlay() throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DataOutputStream out = new DataOutputStream(bytes)) {
            text(out, OVERLAY);
            out.writeInt(2);
            out.writeInt(1);
            text(out, "ravel-live-overlay");
        }
        return bytes.toByteArray();
    }

    private static void text(DataOutputStream out, String value) throws Exception {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        out.writeInt(bytes.length);
        out.write(bytes);
    }

    private static Path marker(String name) throws Exception {
        String configured = System.getProperty("ravel.packet.live");
        check(configured != null && !configured.isBlank(), "missing shared ravel.packet.live directory");
        Path directory = Path.of(configured);
        check(directory.isAbsolute(), "packet handshake directory must be absolute");
        Files.createDirectories(directory);
        return directory.resolve(name);
    }

    private static void schedule(Object owner, Runnable task) throws Exception {
        Method selected = null;
        for (Method method : owner.getClass().getMethods()) {
            if (Arrays.equals(method.getParameterTypes(), new Class<?>[] {Runnable.class})
                    && Future.class.isAssignableFrom(method.getReturnType())) {
                check(selected == null, "ambiguous owning-thread scheduler");
                selected = method;
            }
        }
        check(selected != null, "missing owning-thread scheduler");
        selected.invoke(owner, task);
    }

    private static void deadline(long started, String operation) {
        check(System.nanoTime() - started < TIMEOUT_NANOS, operation + " timed out");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
