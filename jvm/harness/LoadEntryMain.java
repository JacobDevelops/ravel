// Forces one class to load, so the transformer is shown it, and says so.
//
// A class the transformer refuses to rewrite is recorded rather than passed
// over, and the shutdown report is the only place that record is read. Reaching
// it needs the class loaded and nothing else, so this driver loads it by name
// and does nothing with it.
public final class LoadEntryMain {

    private LoadEntryMain() {
    }

    public static void main(String[] arguments) throws ClassNotFoundException {
        System.out.println("loaded=" + Class.forName(arguments[0]).getName());
    }
}
