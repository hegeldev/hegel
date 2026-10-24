package dev.hegel;

import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.Map;

/**
 * Locates a method from its class file, which reflection cannot do: the source file comes from the
 * class's {@code SourceFile} attribute and the line from the method's {@code LineNumberTable}, as
 * the first line of its body that has code.
 */
final class MethodLocation {
    private static final int[] CONSTANT_SIZE = {0, 0, 0, 4, 4, 8, 8, 2, 2, 4, 4, 4, 4, 0, 0, 3, 2, 4, 4, 2, 2};
    private static final int[] CONSTANT_SLOTS = {1, 1, 1, 1, 1, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1};

    private MethodLocation() {}

    /**
     * The location of {@code method}: its source file and the first line of its body, or the
     * conventional file name and line 0 when its class file cannot be read or has no line numbers.
     */
    static TestLocation of(Method method) {
        Class<?> declaring = method.getDeclaringClass();
        String descriptor = MethodType.methodType(method.getReturnType(), method.getParameterTypes())
                .toMethodDescriptorString();
        InputStream classFile =
                declaring.getResourceAsStream("/" + declaring.getName().replace('.', '/') + ".class");
        return of(declaring.getName(), method.getName(), descriptor, classFile);
    }

    /**
     * The location of the method {@code name} with JVM descriptor {@code descriptor} in class
     * {@code className}, read from {@code classFile} ({@code null} when there is none).
     */
    static TestLocation of(String className, String name, String descriptor, InputStream classFile) {
        if (classFile != null) {
            try (DataInputStream in = new DataInputStream(classFile)) {
                ClassFile parsed = new ClassFile(in);
                return TestLocation.of(
                        className, parsed.sourceFile, parsed.firstLines.getOrDefault(name + descriptor, 0), name);
            } catch (IOException | RuntimeException unreadable) {
                return TestLocation.of(className, null, 0, name);
            }
        }
        return TestLocation.of(className, null, 0, name);
    }

    private static final class ClassFile {
        private final DataInputStream in;
        private final String[] utf8;
        private final Map<String, Integer> firstLines = new HashMap<>();
        private String sourceFile;

        ClassFile(DataInputStream in) throws IOException {
            this.in = in;
            in.skipNBytes(8);
            utf8 = new String[in.readUnsignedShort()];
            for (int i = 1; i < utf8.length; ) {
                int tag = in.readUnsignedByte();
                if (tag == 1) {
                    utf8[i] = in.readUTF();
                } else {
                    in.skipNBytes(CONSTANT_SIZE[tag]);
                }
                i += CONSTANT_SLOTS[tag];
            }
            in.skipNBytes(6);
            in.skipNBytes(2L * in.readUnsignedShort());
            members();
            members();
            for (int count = in.readUnsignedShort(); count > 0; count--) {
                String attribute = utf8[in.readUnsignedShort()];
                int length = in.readInt();
                if (attribute.equals("SourceFile")) {
                    sourceFile = utf8[in.readUnsignedShort()];
                } else {
                    in.skipNBytes(length);
                }
            }
        }

        private void members() throws IOException {
            for (int count = in.readUnsignedShort(); count > 0; count--) {
                in.skipNBytes(2);
                String key = utf8[in.readUnsignedShort()] + utf8[in.readUnsignedShort()];
                for (int attributes = in.readUnsignedShort(); attributes > 0; attributes--) {
                    String attribute = utf8[in.readUnsignedShort()];
                    int length = in.readInt();
                    if (attribute.equals("Code")) {
                        code(key);
                    } else {
                        in.skipNBytes(length);
                    }
                }
            }
        }

        private void code(String key) throws IOException {
            in.skipNBytes(4);
            in.skipNBytes(in.readInt());
            in.skipNBytes(8L * in.readUnsignedShort());
            for (int attributes = in.readUnsignedShort(); attributes > 0; attributes--) {
                String attribute = utf8[in.readUnsignedShort()];
                int length = in.readInt();
                if (attribute.equals("LineNumberTable")) {
                    for (int entries = in.readUnsignedShort(); entries > 0; entries--) {
                        in.skipNBytes(2);
                        firstLines.merge(key, in.readUnsignedShort(), Math::min);
                    }
                } else {
                    in.skipNBytes(length);
                }
            }
        }
    }
}
