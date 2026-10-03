This isolated executable imports the actual read-only desktop `model.rs` and
`i18n.rs`. Only the settings `Language` enum is stubbed to avoid compiling GPUI;
the journal structs, serde derives and validation are the desktop implementation.

From the workspace root:

```powershell
cargo run --locked --manifest-path android-app/app/src/test/rust-fixture/Cargo.toml -- generate android-app/app/src/test/resources/desktop-v1.json
cargo run --locked --manifest-path android-app/app/src/test/rust-fixture/Cargo.toml -- validate android-app/app/build/compatibility-roundtrip.json
```

The JVM fixture test compares JSON trees exactly and reopens the durable Kotlin
output. To emit the cross-check file, configure the Gradle test JVM system property
`stillnote.compatibility.output` with an absolute path. The fixture exercises every
log variant, all five statuses, nullable and reciprocal migration links, Unicode,
UUIDs, a collection, and date bounds 0001 and 9999.
