package callback;

public class CallCallback {
    public String method(Callback cb) {
        String name = cb.getName("Ferris");
        return name + " is the name, interop is the game";
    }
}