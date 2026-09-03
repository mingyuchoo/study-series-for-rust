# Finance comparator plugin (WASM)

Build:

```bash
cargo build --release --target wasm32-unknown-unknown --manifest-path plugins/finance/Cargo.toml
```

Use from a comparator spec:

```yaml
fee: { comparator: plugin, plugin: finance_money }
```

and register it in the engine (`ComparisonEngine::register_plugin("finance_money", WasmComparator::from_file(...))`)
or on the CLI: `amap compare --plugin finance_money=plugins/finance/target/wasm32-unknown-unknown/release/amap_plugin_finance.wasm ...`.
