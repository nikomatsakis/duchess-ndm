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

    native String getNameNative(long nativePointer, String input);

    public String getName(String input) {
        return this.getNameNative(nativePointer, input);
    }

    native static void drop(long nativePointer);
}
