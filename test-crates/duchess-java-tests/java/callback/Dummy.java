package callback;

import java.lang.ref.Cleaner;

public class Dummy implements Callback {
    long nativePointer;

    static Cleaner cleaner = Cleaner.create();

    public Dummy(long nativePointer) {
        this.nativePointer = nativePointer;
        cleaner.register(this, () -> {
            drop(nativePointer);
        });
    }

    native String getNameNative(String input, long nativePointer);

    public String getName(String input) {
        return this.getNameNative(input, nativePointer);
    }

    native static void drop(long nativePointer);
}
