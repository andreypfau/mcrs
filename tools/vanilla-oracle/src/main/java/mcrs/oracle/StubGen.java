package mcrs.oracle;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class StubGen {
    public static void main(final String[] args) throws Exception {
        Class<?> root = Class.forName(args[0]);
        Set<Class<?>> all = new LinkedHashSet<>();
        collect(root, all);
        Map<String, Method> abstracts = new LinkedHashMap<>();
        Set<String> defaults = new LinkedHashSet<>();
        for (Class<?> c : all) {
            for (Method m : c.getDeclaredMethods()) {
                if (m.isSynthetic() || Modifier.isStatic(m.getModifiers())) {
                    continue;
                }
                String key = key(m);
                if (m.isDefault()) {
                    defaults.add(key);
                } else if (Modifier.isAbstract(m.getModifiers())) {
                    abstracts.putIfAbsent(key, m);
                }
            }
        }
        for (Map.Entry<String, Method> e : abstracts.entrySet()) {
            if (defaults.contains(e.getKey())) {
                continue;
            }
            Method m = e.getValue();
            List<String> params = new ArrayList<>();
            Class<?>[] types = m.getParameterTypes();
            for (int i = 0; i < types.length; i++) {
                params.add(types[i].getCanonicalName() + " a" + i);
            }
            System.out.println("    @Override");
            System.out.println("    public " + m.getReturnType().getCanonicalName() + " " + m.getName()
                + "(" + String.join(", ", params) + ") {");
            System.out.println("        throw new UnsupportedOperationException(\"" + e.getKey() + "\");");
            System.out.println("    }");
            System.out.println();
        }
    }

    private static String key(final Method m) {
        StringBuilder sb = new StringBuilder(m.getName()).append('(');
        for (Class<?> p : m.getParameterTypes()) {
            sb.append(p.getName()).append(',');
        }
        return sb.append(')').toString();
    }

    private static void collect(final Class<?> c, final Set<Class<?>> out) {
        if (c == null || !out.add(c)) {
            return;
        }
        for (Class<?> i : c.getInterfaces()) {
            collect(i, out);
        }
        collect(c.getSuperclass(), out);
    }
}
