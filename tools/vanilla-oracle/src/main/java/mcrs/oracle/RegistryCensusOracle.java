package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;

public final class RegistryCensusOracle {
    private static final byte[] MAGIC = "MCREGCE0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;

    public static void main(final String[] args) throws Exception {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            )) {
            RegistryAccess.Frozen loaded = PlacementOracle.loadWorldRegistries(resources);
            List<Registry<?>> registries = List.of(
                BuiltInRegistries.ENTITY_TYPE,
                BuiltInRegistries.ITEM,
                BuiltInRegistries.VILLAGER_TYPE,
                BuiltInRegistries.VILLAGER_PROFESSION,
                BuiltInRegistries.ATTRIBUTE,
                loaded.lookupOrThrow(Registries.CAT_VARIANT),
                loaded.lookupOrThrow(Registries.CAT_SOUND_VARIANT)
            );

            Path file = outDir.resolve("registry_census.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
                out.write(MAGIC);
                Bin.i32(out, FORMAT_VERSION);
                Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
                Bin.i32(out, registries.size());
                for (Registry<?> registry : registries) {
                    writeIds(out, registry);
                }
                writeAttributes(out, BuiltInRegistries.ATTRIBUTE);
            }
            System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
        }
    }

    private static <T> void writeIds(final OutputStream out, final Registry<T> registry) throws IOException {
        Bin.str(out, registry.key().identifier().toString());
        Bin.i32(out, registry.size());
        for (int id = 0; id < registry.size(); id++) {
            Bin.str(out, registry.getKey(registry.byId(id)).toString());
        }
        System.out.println(registry.key().identifier() + ": " + registry.size() + " entries");
    }

    private static void writeAttributes(final OutputStream out, final Registry<Attribute> registry) throws IOException {
        Bin.i32(out, registry.size());
        for (int id = 0; id < registry.size(); id++) {
            RangedAttribute attribute = (RangedAttribute)registry.byId(id);
            Bin.str(out, registry.getKey(attribute).toString());
            Bin.i64(out, Double.doubleToRawLongBits(attribute.getDefaultValue()));
            Bin.i64(out, Double.doubleToRawLongBits(attribute.getMinValue()));
            Bin.i64(out, Double.doubleToRawLongBits(attribute.getMaxValue()));
            out.write(attribute.isClientSyncable() ? 1 : 0);
        }
    }
}
