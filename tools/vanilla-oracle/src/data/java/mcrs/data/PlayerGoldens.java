package mcrs.data;

import com.mojang.serialization.Codec;
import java.nio.file.Path;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.RegistryOps;
import net.minecraft.world.ItemStackWithSlot;
import net.minecraft.world.item.ItemStack;

final class PlayerGoldens {
    private PlayerGoldens() {}

    static void vanillaPlayer(final Path current, final Path output) throws Exception {
        CompoundTag player = NbtIo.readCompressed(current, NbtAccounter.unlimitedHeap());
        PacketGoldens.withSession(session -> {
            RegistryOps<Tag> ops = session.access().createSerializationContext(NbtOps.INSTANCE);
            for (String list : new String[] {"Inventory", "EnderItems"}) {
                if (!player.contains(list)) {
                    continue;
                }
                ListTag entries = player.getListOrEmpty(list);
                ListTag rewritten = new ListTag();
                for (int index = 0; index < entries.size(); index++) {
                    CompoundTag entry = entries.getCompoundOrEmpty(index);
                    rewritten.add(reencode(ops, ItemStackWithSlot.CODEC, entry, list, "slot " + entry.getIntOr("Slot", 0)));
                }
                player.put(list, rewritten);
            }
            if (player.contains("equipment")) {
                CompoundTag equipment = player.getCompoundOrEmpty("equipment");
                CompoundTag rewrittenEquipment = new CompoundTag();
                for (String slot : equipment.keySet()) {
                    rewrittenEquipment.put(slot, reencode(ops, ItemStack.CODEC, equipment.get(slot), "equipment", slot));
                }
                player.put("equipment", rewrittenEquipment);
            }
            NbtUtils.addCurrentDataVersion(player);
            NbtIo.writeCompressed(player, output);
        });
    }

    private static <T> Tag reencode(
        final RegistryOps<Tag> ops, final Codec<T> codec, final Tag tag, final String list, final String where
    ) {
        T parsed = codec.parse(ops, tag).getOrThrow(
            message -> new IllegalStateException(list + " " + where + " is no longer accepted: " + message)
        );
        return codec.encodeStart(ops, parsed).getOrThrow(
            message -> new IllegalStateException(list + " " + where + " cannot be written: " + message)
        );
    }
}
