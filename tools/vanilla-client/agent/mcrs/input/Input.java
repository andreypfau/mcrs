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
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import org.lwjgl.sdl.SDLVideo;

/**
 * Plays the input script of one step: the held keys and the view rotation of each client tick.
 * The title {@code Go} starts the script at the tick after the {@code go} tick of the step, and the
 * title {@code Stop} ends it. Outside a script, no key is held.
 */
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
    /** Each line: the first step tick, the held keys, and an optional yaw and pitch. */
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
        LocalPlayer player = client.player;
        for (String[] line : lines) {
            int at = Integer.parseInt(line[0]);
            if (at > tick) {
                break;
            }
            keys = line[1];
            if (at == tick && line.length == 4 && player != null) {
                player.setYRot(Float.parseFloat(line[2]));
                player.setXRot(Float.parseFloat(line[3]));
            }
        }
        hold(client.options, Set.of(keys.split(",")));
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

    private static void hold(Options options, Set<String> keys) {
        KeyMapping[] mappings = {options.keyUp, options.keyDown, options.keyLeft, options.keyRight,
                options.keyJump, options.keyShift, options.keySprint, options.keyUse};
        String[] names = {"forward", "backward", "left", "right", "jump", "shift", "sprint", "use"};
        for (int index = 0; index < names.length; index++) {
            mappings[index].setDown(keys.contains(names[index]));
        }
    }
}
