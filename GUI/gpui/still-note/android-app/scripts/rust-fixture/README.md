This isolated executable imports the actual read-only desktop `model.rs` and
`i18n.rs`. Only the settings `Language` enum is stubbed to avoid compiling GPUI;
the journal structs, serde derives and validation are the desktop implementation.

From the `android-app` project root:

```powershell
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- generate app/testResources/desktop-v1.json
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
```

The JVM fixture test compares JSON trees exactly and reopens the durable Kotlin
output. `app/module.yaml` configures the test JVM system property
`stillnote.compatibility.output`; `kotlin test` emits the cross-check file. The fixture exercises every
log variant, all five statuses, nullable and reciprocal migration links, Unicode,
UUIDs, a collection, and date bounds 0001 and 9999.
