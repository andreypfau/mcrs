package mcrs.oracle;

import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

final class Bin {
    private Bin() {}

    static void i32(final OutputStream out, final int value) throws IOException {
        out.write(value);
        out.write(value >>> 8);
        out.write(value >>> 16);
        out.write(value >>> 24);
    }

    static void i64(final OutputStream out, final long value) throws IOException {
        i32(out, (int)value);
        i32(out, (int)(value >>> 32));
    }

    static void f32(final OutputStream out, final float value) throws IOException {
        i32(out, Float.floatToRawIntBits(value));
    }

    static void str(final OutputStream out, final String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        i32(out, bytes.length);
        out.write(bytes);
    }
}
