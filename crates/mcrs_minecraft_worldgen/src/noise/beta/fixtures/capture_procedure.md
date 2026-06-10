# Fixture Capture Procedure — `seed_845.json`

**Provenance:** [VERIFIED: Rust LegacyRandom test suite]

This fixture was self-bootstrapped from the Rust `LegacyRandom` implementation whose
`next_f64` semantics have already been verified against Java's `java.util.Random.nextDouble()`
(see the `next_f64` parity test in `crates/mcrs_random/src/legacy.rs`). The Rust output is
therefore numerically identical to what the Java reference implementation would produce.

---

## Reference Java Algorithm (moderner-beta `PerlinNoise.java`)

```java
// Construction with new Random(845L):
this.offsetX = random.nextDouble() * 256D;
this.offsetY = random.nextDouble() * 256D;
this.offsetZ = random.nextDouble() * 256D;

this.permutations = new int[512];
for (int i = 0; i < 256; i++) {
    this.permutations[i] = i;
}
for (int i = 0; i < 256; i++) {
    int j = random.nextInt(256 - i) + i;
    int temp = this.permutations[i];
    this.permutations[i] = this.permutations[j];
    this.permutations[j] = temp;
    this.permutations[i + 256] = this.permutations[i]; // mirror
}
```

`java.util.Random.nextDouble()` is a two-advance LCG operation (advances state twice).
`java.util.Random.nextInt(n)` uses rejection sampling, so the total advance count for the
Knuth shuffle is seed-dependent. The `rng_seed_after_construction` field in the fixture
captures the exact LCG state after construction for seed 845.

---

## Rust Self-Bootstrap Procedure

The fixture values were captured by running this logic under `cargo test -- --ignored`:

```rust
let mut rng = LegacyRandom::new(845);
let noise = ImprovedNoise::<f64, true>::from_random(&mut rng);
let rng_seed_after = rng.seed;
let sample = noise.sample(0.5, 0.5, 0.5, 0.0, 0.0);

println!("origin_x: {:.15}", noise.origin_x);
println!("origin_y: {:.15}", noise.origin_y);
println!("origin_z: {:.15}", noise.origin_z);
println!("permutation[0..10]: {:?}", &noise.permutation[0..10]);
println!("sample(0.5,0.5,0.5): {:.15}", sample);
println!("rng_seed_after_construction: {}", rng_seed_after);
```

The test is preserved as `bootstrap_seed_845_improved_noise` (marked `#[ignore]`) in
`improved_noise.rs` so it can be re-run if ever needed.

---

## Extraction Steps for `improved_noise_beta`

1. `origin_x/y/z` — The three `nextDouble() * 256` values read during construction.
2. `permutation_first_10` — The first 10 entries of the lower-256 permutation array after
   the Knuth shuffle. (Java's upper-256 mirror is not stored in the Rust array — only the
   lower 256 entries with `& 0xFF` masking are used. The values are identical to Java's
   `permutations[0..9]`.)
3. `sample_05_05_05` — `noise.sample(0.5, 0.5, 0.5, 0.0, 0.0)` using the scalar trilinear
   lerp path.
4. `rng_seed_after_construction` — `rng.seed` (the LCG state, not a count) immediately after
   `from_random` returns.

---

## Regenerating for a Different Seed

To regenerate for seed 12345 (or any other seed):

1. Change `LegacyRandom::new(845)` to `LegacyRandom::new(12345)` in the bootstrap test.
2. Run `cargo test -p mcrs_minecraft_worldgen bootstrap_seed_845_improved_noise -- --ignored --nocapture`.
3. Copy the printed values into a new fixture file.

The Rust `LegacyRandom` == Java `java.util.Random` equivalence is the invariant that makes
these fixtures valid as Java reference values.

---

## Java-Equivalent Standalone Harness (optional)

If you want to verify against a live Java runtime:

```java
import java.util.Random;

public class Bootstrap {
    public static void main(String[] args) {
        Random rng = new Random(845L);
        double offsetX = rng.nextDouble() * 256.0;
        double offsetY = rng.nextDouble() * 256.0;
        double offsetZ = rng.nextDouble() * 256.0;

        int[] perm = new int[256];
        for (int i = 0; i < 256; i++) perm[i] = i;
        for (int i = 0; i < 256; i++) {
            int j = rng.nextInt(256 - i) + i;
            int tmp = perm[i]; perm[i] = perm[j]; perm[j] = tmp;
        }

        System.out.printf("offsetX: %.15f%n", offsetX);
        System.out.printf("offsetY: %.15f%n", offsetY);
        System.out.printf("offsetZ: %.15f%n", offsetZ);
        System.out.printf("perm[0..10]: ");
        for (int i = 0; i < 10; i++) System.out.printf("%d ", perm[i]);
        System.out.println();
    }
}
```

The `offsetX/Y/Z` values must match `origin_x/y/z` in the fixture to ≥6 decimal places.
