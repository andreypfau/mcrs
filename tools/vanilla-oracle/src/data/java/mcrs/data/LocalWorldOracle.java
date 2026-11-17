package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.List;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.DirectoryLock;
import net.minecraft.util.FileUtil;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.LevelSummary;

public final class LocalWorldOracle {
    private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().setPrettyPrinting().create();
    private static final long FILE_TIME = 1_700_000_000_000L;

    private LocalWorldOracle() {}

    public static void main(final String[] args) throws Exception {
        if (args.length == 2 && args[0].equals("hold")) {
            hold(Path.of(args[1]));
            return;
        }
        if (args.length == 2 && args[0].equals("probe")) {
            System.out.println(DirectoryLock.isLocked(Path.of(args[1])));
            return;
        }
        if (System.getProperty("os.name").startsWith("Windows")) {
            throw new IllegalStateException("the local world capture runs on macOS or Linux only: on Windows the lock of the game denies the read of session.lock through a second handle");
        }
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        Path work = Files.createTempDirectory("local-worlds");
        JsonObject root = new JsonObject();
        root.addProperty("game_version", SharedConstants.getCurrentVersion().id());
        root.add("folder_ids", folderIds(work.resolve("names")));
        root.add("seeds", seeds());
        root.add("summaries", summaries(work.resolve("saves")));
        root.add("lock", lock(work.resolve("lock")));
        root.add("backup", backup(work.resolve("backup")));
        root.add("delete", delete(work.resolve("delete")));
        Files.writeString(out.resolve("local_worlds.json"), GSON.toJson(root) + "\n", StandardCharsets.UTF_8);
        try (Stream<Path> files = Files.walk(work)) {
            for (Path p : files.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(p);
            }
        }
        System.exit(0);
    }

    private static JsonArray folderIds(final Path saves) throws IOException {
        String[][] cases = {
            {"New World"}, {"New World", "New World"}, {"New World", "New World", "New World (1)"},
            {"a.b:c"}, {"con"}, {"COM1"}, {"Map (1)", "Map (1)"}, {"Map (0)"}, {"World"},
            {"x/y\\z*?\"<>|`"},
        };
        JsonArray rows = new JsonArray();
        int n = 0;
        for (String[] c : cases) {
            Path dir = saves.resolve(Integer.toString(n++));
            Files.createDirectories(dir);
            JsonArray existing = new JsonArray();
            for (int i = 1; i < c.length; i++) {
                Files.createDirectory(dir.resolve(c[i]));
                existing.add(c[i]);
            }
            JsonObject row = new JsonObject();
            row.addProperty("name", c[0]);
            row.add("existing", existing);
            row.addProperty("folder_id", FileUtil.findAvailableName(dir, c[0], ""));
            rows.add(row);
        }
        return rows;
    }

    private static JsonArray seeds() {
        String[] texts = {
            "12345", " -7 ", "abc", "9223372036854775808", "0x10", "", "  ", "+5", "-",
            "١٢٣", " 12", "-9223372036854775808",
        };
        JsonArray rows = new JsonArray();
        for (String text : texts) {
            JsonObject row = new JsonObject();
            row.addProperty("text", text);
            var seed = WorldOptions.parseSeed(text);
            if (seed.isPresent()) {
                row.addProperty("seed", seed.getAsLong());
            } else {
                row.add("seed", null);
            }
            rows.add(row);
        }
        return rows;
    }

    private static JsonArray summaries(final Path saves) throws Exception {
        level(saves.resolve("game_type_1/level.dat"), "G1", 1, 19133);
        level(saves.resolve("game_type_7/level.dat"), "G7", 7, 19133);
        level(saves.resolve("game_type_minus_1/level.dat"), "Gm1", -1, 19133);
        level(saves.resolve("game_type_missing/level.dat"), "Gnone", null, 19133);
        level(saves.resolve("backup_only/level.dat_old"), "OldOnly", 1, 19133);
        level(saves.resolve("storage_19134/level.dat"), "BadVer", 0, 19134);
        level(saves.resolve("storage_19132/level.dat"), "V19132", 0, 19132);
        level(saves.resolve("empty_name/level.dat"), "", 0, 19133);
        Files.createDirectories(saves.resolve("region_only/region"));
        Files.writeString(saves.resolve("regular_file"), "x");
        try (Stream<Path> files = Files.walk(saves)) {
            for (Path p : files.toList()) {
                Files.setLastModifiedTime(p, FileTime.fromMillis(FILE_TIME));
            }
        }
        LevelStorageSource source = LevelStorageSource.createDefault(saves);
        List<LevelSummary> list = source.loadLevelSummaries(source.findLevelCandidates()).get().stream()
            .sorted(Comparator.comparing(LevelSummary::getLevelId))
            .toList();
        JsonArray rows = new JsonArray();
        for (LevelSummary s : list) {
            JsonObject row = new JsonObject();
            row.addProperty("folder", s.getLevelId());
            boolean readable = !(s instanceof LevelSummary.CorruptedLevelSummary);
            row.addProperty("readable", readable);
            row.addProperty("shown_name", s.getLevelName());
            row.addProperty("last_played", s.getLastPlayed());
            row.addProperty("disabled", s.isDisabled());
            row.addProperty("info", s.getInfo().getString());
            if (readable) {
                row.addProperty("game_mode", s.getGameMode().getName());
                row.addProperty("commands", s.hasCommands());
            }
            rows.add(row);
        }
        return rows;
    }

    private static JsonObject lock(final Path dir) throws Exception {
        JsonObject result = new JsonObject();
        Path world = dir.resolve("world");
        Path file = world.resolve("session.lock");
        DirectoryLock lock = DirectoryLock.create(world);
        result.addProperty("other_process_sees_held_lock", probe(world));
        result.addProperty("bytes", HexFormat.of().formatHex(Files.readAllBytes(file)));
        result.addProperty("other_process_sees_lock_after_second_handle_closes", probe(world));
        lock.close();
        result.addProperty("bytes_after_release", HexFormat.of().formatHex(Files.readAllBytes(file)));
        result.addProperty("file_kept_after_release", Files.exists(file));
        result.addProperty("other_process_sees_released_lock", probe(world));
        Process holder = holder(world);
        result.addProperty("test_while_other_process_holds", DirectoryLock.isLocked(world));
        result.addProperty("take_while_other_process_holds", take(world));
        release(holder);
        result.addProperty("take_after_other_process_releases", take(world));
        result.addProperty("test_without_file", DirectoryLock.isLocked(dir.resolve("none")));
        file.toFile().setWritable(false);
        result.addProperty("test_of_file_not_writable", DirectoryLock.isLocked(world));
        file.toFile().setWritable(true);
        return result;
    }

    private static JsonObject backup(final Path saves) throws Exception {
        level(saves.resolve("world/level.dat_old"), "Backup", 1, 19133);
        JsonObject result = new JsonObject();
        try (var access = LevelStorageSource.createDefault(saves).validateAndCreateAccess("world")) {
            result.addProperty("level_name", access.getUnfixedDataTagWithFallback().get("LevelName").asString(""));
        }
        result.add("files_after", files(saves.resolve("world")));
        return result;
    }

    private static JsonObject delete(final Path saves) throws Exception {
        Path world = saves.resolve("world");
        level(world.resolve("level.dat"), "Delete", 0, 19133);
        Files.createDirectories(world.resolve("region"));
        Files.writeString(world.resolve("region/r.0.0.mca"), "x");
        DirectoryLock.create(world).close();
        LevelStorageSource source = LevelStorageSource.createDefault(saves);
        JsonObject result = new JsonObject();
        Process holder = holder(world);
        result.addProperty("delete_while_other_process_holds", attemptDelete(source));
        result.add("files_after_refused_delete", files(world));
        release(holder);
        result.addProperty("delete_after_release", attemptDelete(source));
        result.addProperty("folder_exists_after_delete", Files.exists(world));
        return result;
    }

    private static String attemptDelete(final LevelStorageSource source) {
        try (var access = source.createAccess("world")) {
            access.deleteLevel();
            return "deleted";
        } catch (IOException e) {
            return "refused";
        }
    }

    private static String take(final Path world) {
        try {
            DirectoryLock.create(world).close();
            return "taken";
        } catch (IOException e) {
            return "refused";
        }
    }

    private static JsonArray files(final Path dir) throws IOException {
        JsonArray names = new JsonArray();
        try (Stream<Path> files = Files.walk(dir)) {
            files.filter(p -> !p.equals(dir)).map(p -> dir.relativize(p).toString().replace('\\', '/')).sorted().forEach(names::add);
        }
        return names;
    }

    private static void level(final Path file, final String name, final Integer gameType, final int storage) throws IOException {
        CompoundTag data = new CompoundTag();
        data.putString("LevelName", name);
        if (gameType != null) {
            data.putInt("GameType", gameType);
        }
        data.putInt("version", storage);
        int dataVersion = SharedConstants.getCurrentVersion().dataVersion().version();
        data.putInt("DataVersion", dataVersion);
        data.putLong("LastPlayed", 1000L);
        CompoundTag version = new CompoundTag();
        version.putInt("Id", dataVersion);
        version.putString("Name", SharedConstants.getCurrentVersion().name());
        version.putString("Series", "main");
        version.putByte("Snapshot", (byte) (SharedConstants.getCurrentVersion().stable() ? 0 : 1));
        data.put("Version", version);
        CompoundTag root = new CompoundTag();
        root.put("Data", data);
        Files.createDirectories(file.getParent());
        NbtIo.writeCompressed(root, file);
    }

    private static void hold(final Path world) throws IOException {
        try (DirectoryLock lock = DirectoryLock.create(world)) {
            System.out.println("held");
            System.out.flush();
            while (System.in.read() >= 0) {
            }
        }
    }

    private static List<String> child(final String mode, final Path world) {
        String java = Path.of(System.getProperty("java.home"), "bin", "java").toString();
        return List.of(java, "-cp", System.getProperty("java.class.path"), LocalWorldOracle.class.getName(), mode, world.toString());
    }

    private static Process holder(final Path world) throws IOException {
        Process p = new ProcessBuilder(child("hold", world)).redirectError(ProcessBuilder.Redirect.INHERIT).start();
        String line = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8)).readLine();
        if (!"held".equals(line)) {
            throw new IOException("the holder process did not take the lock: " + line);
        }
        return p;
    }

    private static void release(final Process holder) throws Exception {
        holder.getOutputStream().close();
        holder.waitFor();
    }

    private static boolean probe(final Path world) throws Exception {
        Process p = new ProcessBuilder(child("probe", world)).redirectError(ProcessBuilder.Redirect.INHERIT).start();
        String line = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8)).readLine();
        int status = p.waitFor();
        if (status != 0 || !("true".equals(line) || "false".equals(line))) {
            throw new IOException("the probe process failed: exit status " + status + ", output " + line);
        }
        return line.equals("true");
    }
}
