// Loads a target so the transformer's shutdown report can record a refusal.
public final class LoadEntryMain {

    private LoadEntryMain() {
    }

    public static void main(String[] arguments) throws ClassNotFoundException {
        System.out.println("loaded=" + Class.forName(arguments[0]).getName());
    }
}
