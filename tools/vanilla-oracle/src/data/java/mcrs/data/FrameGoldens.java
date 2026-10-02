package mcrs.data;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Supplier;
import net.minecraft.network.CompressionEncoder;
import net.minecraft.network.Varint21LengthFieldPrepender;

public final class FrameGoldens {
    private static final HexFormat HEX = HexFormat.of();
    private static final int NO_COMPRESSION = -1;

    private FrameGoldens() {}

    private static final class Compressor extends CompressionEncoder {
        Compressor(final int threshold) {
            super(threshold);
        }

        void compress(final ByteBuf body, final ByteBuf out) {
            encode(null, body, out);
        }
    }

    private static final class Prepender extends Varint21LengthFieldPrepender {
        void prepend(final ByteBuf body, final ByteBuf out) {
            encode(null, body, out);
        }
    }

    private static byte[] body(final int size) {
        byte[] body = new byte[size];
        for (int i = 0; i < size; i++) {
            body[i] = (byte) (i % 251);
        }
        return body;
    }

    private static byte[] frame(final int threshold, final byte[] body) {
        ByteBuf framed = Unpooled.buffer();
        ByteBuf source = Unpooled.wrappedBuffer(body);
        if (threshold == NO_COMPRESSION) {
            new Prepender().prepend(source, framed);
        } else {
            ByteBuf compressed = Unpooled.buffer();
            new Compressor(threshold).compress(source, compressed);
            new Prepender().prepend(compressed, framed);
        }
        byte[] bytes = new byte[framed.readableBytes()];
        framed.readBytes(bytes);
        return bytes;
    }

    private static String single(final int threshold, final int size) {
        return HEX.formatHex(frame(threshold, body(size)));
    }

    static void frames(final Path current, final Path output) throws Exception {
        Map<String, Supplier<String>> labels = new LinkedHashMap<>();
        labels.put("t256_256", () -> single(256, 256));
        StringBuilder text = new StringBuilder();
        labels.forEach((label, hex) -> text.append(label).append(' ').append(hex.get()).append('\n'));
        Files.writeString(output, text.toString(), StandardCharsets.UTF_8);
    }
}
