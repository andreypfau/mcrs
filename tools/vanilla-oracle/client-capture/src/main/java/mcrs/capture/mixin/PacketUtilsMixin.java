package mcrs.capture.mixin;

import mcrs.capture.EntityLog;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.PacketListener;
import net.minecraft.network.PacketProcessor;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketUtils;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(PacketUtils.class)
abstract class PacketUtilsMixin {
	@Inject(
		method = "ensureRunningOnSameThread(Lnet/minecraft/network/protocol/Packet;Lnet/minecraft/network/PacketListener;Lnet/minecraft/network/PacketProcessor;)V",
		at = @At("HEAD")
	)
	private static void mcrs$logApplied(Packet<?> packet, PacketListener listener, PacketProcessor processor, CallbackInfo ci) {
		if (listener instanceof ClientPacketListener client && processor.isSameThread()) {
			EntityLog.packet(client, packet);
		}
	}
}
