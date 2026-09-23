package mcrs.uat;

import com.google.gson.GsonBuilder;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/// Joins the server named by `mcrs.uat.address`, drives every drag and
/// double-click scenario through the vanilla click path, and exits with the
/// verdict written to `mcrs.uat.report`.
public final class InventoryUat implements ClientModInitializer {
    private static final int OUTSIDE = AbstractContainerMenu.SLOT_CLICKED_OUTSIDE;
    private static final int HOTBAR_0 = 36;
    private static final int SETTLE = 10;

    private final Deque<Step> steps = new ArrayDeque<>();
    private final List<Map<String, Object>> results = new ArrayList<>();
    private Minecraft mc;
    private Step current;
    private int countdown;
    private boolean connecting;
    private int joinedTicks;
    private int idleTicks;

    private record Step(int delay, Runnable run) {}

    @Override
    public void onInitializeClient() {
        ClientTickEvents.END_CLIENT_TICK.register(this::tick);
    }

    private void tick(Minecraft client) {
        mc = client;
        if (client.player == null || client.level == null) {
            boolean menuReady = client.gui.screen() instanceof TitleScreen || ++idleTicks > 100;
            if (!connecting && client.gui.screen() != null && menuReady) {
                connect();
            }
            return;
        }
        if (joinedTicks++ == 40) {
            schedule();
        }
        if (current == null) {
            current = steps.poll();
            if (current == null) {
                return;
            }
            countdown = current.delay;
        }
        if (countdown-- == 0) {
            current.run.run();
            current = null;
        }
    }

    private void connect() {
        connecting = true;
        String address = System.getProperty("mcrs.uat.address", "127.0.0.1:25565");
        ConnectScreen.startConnecting(
            mc.gui.screen(),
            mc,
            ServerAddress.parseString(address),
            new ServerData("mcrs", address, ServerData.Type.OTHER),
            false,
            null
        );
    }

    private void schedule() {
        scenario("left drag splits 64 over five slots", () -> {
            give(HOTBAR_0, 64);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            drag(0, 9, 10, 11, 12, 13);
        }, expect(4, 9, 12, 10, 12, 11, 12, 12, 12, 13, 12));

        scenario("right drag places one per slot", () -> {
            give(HOTBAR_0, 64);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            drag(1, 9, 10, 11, 12, 13);
        }, expect(59, 9, 1, 10, 1, 11, 1, 12, 1, 13, 1));

        scenario("creative middle drag fills every slot and empties the cursor", () -> {
            give(HOTBAR_0, 64);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            drag(2, 9, 10, 11, 12, 13);
        }, expect(0, 9, 64, 10, 64, 11, 64, 12, 64, 13, 64));

        scenario("drag released over one slot is a plain click", () -> {
            give(HOTBAR_0, 64);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            drag(0, 9);
        }, expect(0, 9, 64));

        scenario("double-click gathers forwards, partial stacks first", () -> {
            give(HOTBAR_0, 5);
            give(9, 40);
            give(10, 64);
            give(11, 40);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            click(HOTBAR_0, 0, ContainerInput.PICKUP_ALL);
        }, expect(64, 9, 0, 10, 64, 11, 21));

        scenario("double-click gathers backwards with the right button", () -> {
            give(HOTBAR_0, 5);
            give(9, 40);
            give(10, 64);
            give(11, 40);
        }, () -> {
            click(HOTBAR_0, 0, ContainerInput.PICKUP);
            click(HOTBAR_0, 1, ContainerInput.PICKUP_ALL);
        }, expect(64, 9, 21, 10, 64, 11, 0));

        steps.add(new Step(SETTLE, this::finish));
    }

    private void scenario(String name, Runnable setup, Runnable act, Map<Integer, Integer> expected) {
        steps.add(new Step(SETTLE, this::clear));
        steps.add(new Step(SETTLE, setup));
        steps.add(new Step(SETTLE, () -> {
            Probe.reset();
            Probe.armed = true;
            act.run();
        }));
        steps.add(new Step(SETTLE * 2, () -> {
            Probe.armed = false;
            record(name, expected);
        }));
    }

    /// `carried, slot, count, slot, count, ...`
    private static Map<Integer, Integer> expect(int carried, int... slotCounts) {
        Map<Integer, Integer> expected = new LinkedHashMap<>();
        expected.put(-1, carried);
        for (int i = 0; i < slotCounts.length; i += 2) {
            expected.put(slotCounts[i], slotCounts[i + 1]);
        }
        return expected;
    }

    private void record(String name, Map<Integer, Integer> expected) {
        List<String> mismatches = new ArrayList<>();
        for (Map.Entry<Integer, Integer> entry : expected.entrySet()) {
            int actual = entry.getKey() == -1 ? carried() : count(entry.getKey());
            if (actual != entry.getValue()) {
                String where = entry.getKey() == -1 ? "cursor" : "slot " + entry.getKey();
                mismatches.add(where + ": expected " + entry.getValue() + ", client has " + actual);
            }
        }
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("name", name);
        result.put("passed", mismatches.isEmpty() && Probe.resyncs == 0 && Probe.corrections.isEmpty());
        result.put("mismatches", mismatches);
        result.put("full_resyncs", Probe.resyncs);
        result.put("server_corrections", List.copyOf(Probe.corrections));
        results.add(result);
        System.out.println("UAT " + (result.get("passed").equals(true) ? "PASS" : "FAIL") + ": " + name + " " + result);
    }

    private void clear() {
        if (carried() > 0) {
            click(HOTBAR_0 + 8, 0, ContainerInput.PICKUP);
        }
        for (int slot = 9; slot <= 45; slot++) {
            give(slot, 0);
        }
    }

    private void give(int slot, int count) {
        ItemStack stack = count == 0 ? ItemStack.EMPTY : new ItemStack(Items.STONE, count);
        mc.player.inventoryMenu.getSlot(slot).set(stack.copy());
        mc.gameMode.handleCreativeModeItemAdd(stack, slot);
    }

    private void click(int slot, int button, ContainerInput input) {
        mc.gameMode.handleContainerInput(0, slot, button, input, mc.player);
    }

    private void drag(int type, int... slots) {
        click(OUTSIDE, AbstractContainerMenu.getQuickcraftMask(0, type), ContainerInput.QUICK_CRAFT);
        for (int slot : slots) {
            click(slot, AbstractContainerMenu.getQuickcraftMask(1, type), ContainerInput.QUICK_CRAFT);
        }
        click(OUTSIDE, AbstractContainerMenu.getQuickcraftMask(2, type), ContainerInput.QUICK_CRAFT);
    }

    private int count(int slot) {
        return mc.player.inventoryMenu.getSlot(slot).getItem().getCount();
    }

    private int carried() {
        return mc.player.inventoryMenu.getCarried().getCount();
    }

    private void finish() {
        boolean passed = results.stream().allMatch(result -> result.get("passed").equals(true));
        Map<String, Object> report = new LinkedHashMap<>();
        report.put("passed", passed);
        report.put("scenarios", results);
        String json = new GsonBuilder().setPrettyPrinting().create().toJson(report);
        System.out.println("UAT REPORT\n" + json);
        try {
            Path path = Path.of(System.getProperty("mcrs.uat.report", "uat-report.json"));
            Files.createDirectories(path.toAbsolutePath().getParent());
            Files.writeString(path, json);
        } catch (IOException e) {
            e.printStackTrace();
        }
        System.exit(passed ? 0 : 1);
    }
}
