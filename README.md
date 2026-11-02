# cargo-sbom-precursor

Types for reading the SBOM precursor files Cargo writes with `-Z sbom`.

```console
$ CARGO_BUILD_SBOM=true cargo +nightly build -Z sbom
```

```rust
use cargo_sbom_precursor::Precursor;

let precursor = Precursor::from_path("target/debug/app.cargo-sbom.json")?;
for dep in &precursor.root_crate().dependencies {
    println!("{} ({:?})", precursor.crates[dep.index].id, dep.kind);
}
```

The format is unstable, see the [Cargo documentation](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#sbom)
and the [tracking issue](https://github.com/rust-lang/cargo/issues/16565).

<hr/>

Have fun!
