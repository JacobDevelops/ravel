package dev.ravel.jvm;

/** Local field provenance fixtures for the legacy global-palette size rewrite. */
public final class PaletteFixture {
    public static final Mapper STATES = new Mapper();
    public static final Mapper OTHER = new Mapper();
    public int bits;
    public static final class Mapper {
        public int size() { return 13; }
    }
    public void resize(int ignored) {
        bits = Integer.SIZE - Integer.numberOfLeadingZeros(STATES.size() - 1);
    }
    public void absent(int ignored) { bits = 7; }
    public void twice(int ignored) { bits = STATES.size() + STATES.size(); }
    public void wrong(int ignored) { bits = OTHER.size(); }
}
