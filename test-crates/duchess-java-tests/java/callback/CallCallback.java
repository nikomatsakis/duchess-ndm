package callback;

public class CallCallback {
    public String method(Callback cb) {
        String name = cb.getName("Ferris");
        int age = cb.getAge();
        return name + " is " + age + " years old.";
    }
}