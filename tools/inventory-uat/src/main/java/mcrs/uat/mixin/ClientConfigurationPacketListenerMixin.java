package mcrs.uat.mixin;

import mcrs.uat.Probe;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientConfigurationPacketListenerImpl;
import net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ClientConfigurationPacketListenerImpl.class)
public class ClientConfigurationPacketListenerMixin {
    // The handler runs once on the network thread, which reschedules it, and once on the
    // main thread; counting only the main-thread call counts each packet once.
    @Inject(method = "handleRegistryData", at = @At("HEAD"))
    private void uat$registryData(ClientboundRegistryDataPacket packet, CallbackInfo ci) {
        if (Minecraft.getInstance().isSameThread()) {
            Probe.registries++;
        }
    }
}
