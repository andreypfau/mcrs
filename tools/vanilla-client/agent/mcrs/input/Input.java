package mcrs.input;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.List;
import java.util.Set;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.screens.InBedChatScreen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.inventory.ContainerInput;
import org.lwjgl.sdl.SDLVideo;

public final class Input {

    private static final boolean MINIMIZE = Boolean.getBoolean("mcrs.minimize");
    private static final Field TITLE;

    static {
        try {
            TITLE = Hud.class.getDeclaredField("title");
            TITLE.setAccessible(true);
        } catch (ReflectiveOperationException exception) {
            throw new IllegalStateException(exception);
        }
    }

    private static Object seen;
    private static boolean minimized;
    private static int tick;
    private static List<String[]> lines = List.of();

    private Input() {}

    public static void tick() throws ReflectiveOperationException {
        Minecraft client = Minecraft.getInstance();
        if (MINIMIZE && !minimized) {
            minimized = SDLVideo.SDL_MinimizeWindow(client.getWindow().handle());
        }
        Object shown = TITLE.get(client.gui.hud);
        if (shown != seen) {
            seen = shown;
            String text = shown instanceof Component component ? component.getString() : "";
            if (text.equals("Go")) {
                start();
            } else if (text.equals("Stop")) {
                lines = List.of();
            }
        }
        tick++;
        String keys = "none";
        Set<String> pressed = Set.of();
        LocalPlayer player = client.player;
        for (String[] line : lines) {
            int at = Integer.parseInt(line[0]);
            if (at > tick) {
                break;
            }
            keys = line[1];
            pressed = at == tick ? Set.of(keys.split(",")) : Set.of();
            if (at == tick && line.length == 4 && player != null) {
                player.setYRot(Float.parseFloat(line[2]));
                player.setXRot(Float.parseFloat(line[3]));
            }
        }
        hold(client.options, Set.of(keys.split(",")));
        if (pressed.contains("leave_bed") && client.gui.screen() instanceof InBedChatScreen bed) {
            bed.onClose();
        }
        press(client, pressed);
    }

    private static void start() {
        List<String> text;
        try {
            text = Files.readAllLines(Agent.script);
        } catch (IOException exception) {
            throw new UncheckedIOException(exception);
        }
        tick = Integer.parseInt(text.getFirst().substring("go ".length()));
        lines = text.stream().skip(1).filter(line -> !line.isBlank())
                .map(line -> line.trim().split(" +")).toList();
        System.out.println("input: Go at step tick " + (tick + 1) + ", " + lines.size() + " lines: "
                + lines.stream().map(Arrays::toString).toList());
    }

    private static void press(Minecraft client, Set<String> pressed) {
        String[] names = {"drop", "swap_hands", "inventory"};
        String[] mappings = {"key.drop", "key.swapOffhand", "key.inventory"};
        for (int index = 0; index < names.length; index++) {
            if (pressed.contains(names[index])) {
                KeyMapping.click(KeyMapping.get(mappings[index]).getDefaultKey());
                System.out.println("input: " + names[index] + " at step tick " + tick);
            }
        }
        if (pressed.contains("close") && client.gui.screen() != null) {
            client.gui.screen().onClose();
        }
        LocalPlayer player = client.player;
        for (String key : pressed) {
            if (!key.startsWith("click:") || player == null) {
                continue;
            }
            if (!(client.gui.screen() instanceof AbstractContainerScreen<?>)) {
                System.out.println("input: no container screen for " + key);
                continue;
            }
            int slot = Integer.parseInt(key.substring("click:".length()));
            client.gameMode.handleContainerInput(player.containerMenu.containerId, slot, 0,
                    ContainerInput.PICKUP, player);
            System.out.println("input: primary click on slot " + slot + " at step tick " + tick);
        }
    }

    private static void hold(Options options, Set<String> keys) {
        KeyMapping[] mappings = {options.keyUp, options.keyDown, options.keyLeft, options.keyRight,
                options.keyJump, options.keyShift, options.keySprint, options.keyUse};
        String[] names = {"forward", "backward", "left", "right", "jump", "shift", "sprint", "use"};
        for (int index = 0; index < names.length; index++) {
            mappings[index].setDown(keys.contains(names[index]));
        }
    }
}
