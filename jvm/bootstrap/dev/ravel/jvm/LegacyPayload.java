package dev.ravel.jvm;

import java.nio.charset.StandardCharsets;

/** Selects the requeued main-thread call without consuming vanilla's buffer. */
final class LegacyPayload {
    static final int MAX_BYTES = 32767;
    static final int MAX_CHANNEL_BYTES = 20;
    private static final java.util.concurrent.atomic.AtomicLong CLIENT_SELECTIONS =
            LegacyRuntime.PROOF ? new java.util.concurrent.atomic.AtomicLong() : null;
    private static final java.util.concurrent.atomic.AtomicLong SERVER_SELECTIONS =
            LegacyRuntime.PROOF ? new java.util.concurrent.atomic.AtomicLong() : null;

    private LegacyPayload() {
    }

    static void receive(Object handler, Object packet, boolean toServer) {
        try {
            select(handler, packet, toServer);
        } catch (ReflectiveOperationException | RuntimeException failure) {
            Agent.diagnose("legacy payload refused: " + Agent.unusableName(failure));
        }
    }

    private static void select(Object handler, Object packet, boolean toServer)
            throws ReflectiveOperationException {
        String handlerRole = toServer ? "LEGACY_SERVER_HANDLER" : "LEGACY_CLIENT_HANDLER";
        String ownerField = toServer ? "server" : "minecraft";
        Object owner = LegacyAccess.field(handlerRole, ownerField, handler);
        if (!Boolean.TRUE.equals(LegacyAccess.call(
                "LEGACY_EVENT_LOOP", "isOnSameThread", owner))) {
            return;
        }
        String packetRole = toServer ? "LEGACY_PACKET_SERVER" : "LEGACY_PACKET_CLIENT";
        String channel = (String) LegacyAccess.call(packetRole, "getChannel", packet);
        if (!validChannel(channel) || !Hook.ravelLegacyPacketOwner(channel)) {
            return;
        }
        Object buffer = LegacyAccess.call(packetRole, "getData", packet);
        int length = (Integer) LegacyAccess.call("LEGACY_BUFFER", "readableBytes", buffer);
        if (length < 0 || length > MAX_BYTES) {
            Agent.diagnose("refused oversized legacy payload on " + channel);
            return;
        }
        int index = (Integer) LegacyAccess.call("LEGACY_BUFFER", "readerIndex", buffer);
        byte[] bytes = new byte[length];
        LegacyAccess.call("LEGACY_BUFFER", "getBytes", buffer, index, bytes);
        Hook.ravelLegacyPacket(channel, bytes, toServer);
        if (LegacyRuntime.PROOF) {
            (toServer ? SERVER_SELECTIONS : CLIENT_SELECTIONS).incrementAndGet();
        }
    }

    static long selections(boolean toServer) {
        return LegacyRuntime.PROOF ? (toServer ? SERVER_SELECTIONS : CLIENT_SELECTIONS).get() : 0;
    }

    static boolean validChannel(String channel) {
        if (channel == null || channel.isEmpty() || channel.length() > MAX_CHANNEL_BYTES
                || channel.getBytes(StandardCharsets.UTF_8).length > MAX_CHANNEL_BYTES) {
            return false;
        }
        int colon = channel.indexOf(':');
        if (colon <= 0 || colon == channel.length() - 1) {
            return false;
        }
        for (int at = 0; at < channel.length(); at++) {
            char value = channel.charAt(at);
            boolean common = value >= 'a' && value <= 'z' || value >= '0' && value <= '9'
                    || value == '_' || value == '-' || value == '.';
            if (!common && !(at == colon && value == ':') && !(at > colon && value == '/')) {
                return false;
            }
        }
        return true;
    }
}
