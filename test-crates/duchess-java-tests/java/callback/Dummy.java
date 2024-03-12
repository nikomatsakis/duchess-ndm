import java.lang.ref.Cleaner;

import duchess.RustReference;

public class Dummy implements Callback {
    long nativePointer;

    static Cleaner cleaner = Cleaner.create();

    public Dummy(long nativePointer) {
        this.nativePointer = nativePosinter;
        cleaner.register(this, () -> {
            drop(nativePointer);
        });
    }

    native String getNameNative(long nativePointer, String input);

    public String getName(String input) {
        return this.getNameNative(nativePointer, input);
    }

    native int getAgeNative(long nativePointer);

    public int getAge() {
        return this.getAgeNative(nativePointer);
    }

    native static void drop(long nativePointer);
}
