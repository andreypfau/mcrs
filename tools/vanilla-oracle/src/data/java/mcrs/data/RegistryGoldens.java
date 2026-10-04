package mcrs.data;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.TreeSet;
import mcrs.data.CodecGoldens.Session;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;

final class RegistryGoldens {
    private RegistryGoldens() {}

    static void registryValues(final Path current, final Path output) throws Exception {
        TreeSet<String> names = new TreeSet<>();
        for (String line : Files.readAllLines(current, StandardCharsets.UTF_8)) {
            if (!line.isBlank()) {
                names.add(line.split(" ", 2)[0]);
            }
        }
        CodecGoldens.withSession(session -> {
            List<String> out = new ArrayList<>();
            for (String name : names) {
                out.addAll(entries(session, name, synchronizedRegistry(Identifier.parse(name), name)));
            }
            CodecGoldens.writeLines(output, out);
        });
    }

    private static RegistryDataLoader.RegistryData<?> synchronizedRegistry(final Identifier id, final String name) {
        return RegistryDataLoader.SYNCHRONIZED_REGISTRIES.stream()
            .filter(data -> data.key().identifier().equals(id))
            .findFirst()
            .orElseThrow(() -> new IllegalStateException("registry " + name + " is not synchronized"));
    }

    private static <T> List<String> entries(
        final Session session, final String name, final RegistryDataLoader.RegistryData<T> data
    ) {
        Registry<T> registry = session.access().lookupOrThrow(data.key());
        List<Holder.Reference<T>> holders = new ArrayList<>();
        registry.listElements().forEach(holders::add);
        holders.sort(Comparator.comparing(holder -> holder.key().identifier().toString()));

        List<String> lines = new ArrayList<>();
        for (Holder.Reference<T> holder : holders) {
            Tag tag = data.elementCodec()
                .encodeStart(session.nbt(), holder.value())
                .getOrThrow(message -> new IllegalStateException(
                    "failed to encode " + name + " " + holder.key().identifier() + ": " + message
                ));
            lines.add(name + " " + holder.key().identifier() + " " + tag);
        }
        if (lines.isEmpty()) {
            lines.add(name);
        }
        return lines;
    }
}
