package mcrs.uat.mixin;

import mcrs.uat.Probe;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket;
import net.minecraft.network.protocol.game.ClientboundContainerSetSlotPacket;
import net.minecraft.network.protocol.game.ClientboundSetCursorItemPacket;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ClientPacketListener.class)
public class ClientPacketListenerMixin {
    // Each handler runs once on the network thread (which reschedules it) and once
    // on the main thread; only the main-thread call sees the client state.
    private static boolean uat$skip() {
        return !Probe.armed || !Minecraft.getInstance().isSameThread() || Minecraft.getInstance().player == null;
    }

    @Inject(method = "handleContainerContent", at = @At("HEAD"))
    private void uat$content(ClientboundContainerSetContentPacket packet, CallbackInfo ci) {
        if (uat$skip() || packet.containerId() != 0) {
            return;
        }
        Probe.resyncs++;
    }

    @Inject(method = "handleContainerSetSlot", at = @At("HEAD"))
    private void uat$slot(ClientboundContainerSetSlotPacket packet, CallbackInfo ci) {
        if (uat$skip() || packet.getContainerId() != 0) {
            return;
        }
        ItemStack had = Minecraft.getInstance().player.inventoryMenu.getSlot(packet.getSlot()).getItem();
        if (!ItemStack.matches(had, packet.getItem())) {
            Probe.corrections.add("slot " + packet.getSlot() + ": client " + had + " -> server " + packet.getItem());
        }
    }

    @Inject(method = "handleSetCursorItem", at = @At("HEAD"))
    private void uat$cursor(ClientboundSetCursorItemPacket packet, CallbackInfo ci) {
        if (uat$skip()) {
            return;
        }
        ItemStack had = Minecraft.getInstance().player.containerMenu.getCarried();
        if (!ItemStack.matches(had, packet.contents())) {
            Probe.corrections.add("cursor: client " + had + " -> server " + packet.contents());
        }
    }
}
