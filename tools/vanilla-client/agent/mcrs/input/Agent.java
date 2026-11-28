package mcrs.input;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.nio.file.Path;
import java.security.ProtectionDomain;

public final class Agent {
    private static final String CLIENT = "net/minecraft/client/Minecraft";

    static Path script;

    private Agent() {}

    public static void premain(String argument, Instrumentation instrumentation) {
        script = Path.of(argument);
        instrumentation.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined,
                    ProtectionDomain domain, byte[] bytes) {
                if (!CLIENT.equals(name)) {
                    return null;
                }
                try {
                    return hook(bytes);
                } catch (RuntimeException exception) {
                    exception.printStackTrace();
                    throw exception;
                }
            }
        });
    }

    private static byte[] hook(byte[] bytes) {
        ClassFile file = ClassFile.of();
        CodeTransform call = new CodeTransform() {
            @Override
            public void atStart(CodeBuilder builder) {
                builder.invokestatic(ClassDesc.of("mcrs.input.Input"), "tick",
                        MethodTypeDesc.of(ConstantDescs.CD_void));
            }

            @Override
            public void accept(CodeBuilder builder, CodeElement element) {
                builder.with(element);
            }
        };
        return file.transformClass(file.parse(bytes), ClassTransform.transformingMethodBodies(
                method -> method.methodName().equalsString("tick")
                        && method.methodType().equalsString("()V"),
                call));
    }
}
