package mcrs.uat;

import java.util.ArrayList;
import java.util.List;

/// What the server sent back while a scenario was armed: every full container
/// resend and every slot or cursor value that differed from what the client had
/// already predicted. A vanilla server produces none of either for a legal drag.
public final class Probe {
    public static boolean armed;
    public static int resyncs;
    public static final List<String> corrections = new ArrayList<>();

    public static void reset() {
        resyncs = 0;
        corrections.clear();
    }

    private Probe() {}
}
