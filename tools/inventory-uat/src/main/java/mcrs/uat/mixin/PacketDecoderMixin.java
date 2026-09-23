package mcrs.uat.mixin;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandlerContext;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.network.PacketDecoder;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(PacketDecoder.class)
public class PacketDecoderMixin {
    private static boolean uat$dumped;

    @Inject(method = "decode", at = @At("HEAD"))
    private void uat$dump(ChannelHandlerContext ctx, ByteBuf input, List<Object> out, CallbackInfo ci) throws Exception {
        String dump = System.getProperty("mcrs.uat.dumpPacket");
        if (dump == null || uat$dumped || input.readableBytes() == 0) {
            return;
        }
        int id = Integer.parseInt(dump.substring(dump.indexOf(':') + 1), 16);
        if ((input.getByte(input.readerIndex()) & 0xFF) != id) {
            return;
        }
        uat$dumped = true;
        byte[] bytes = new byte[input.readableBytes()];
        input.getBytes(input.readerIndex(), bytes);
        Files.write(Path.of(dump.substring(0, dump.indexOf(':'))), bytes);
    }
}
