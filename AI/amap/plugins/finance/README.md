# Finance comparator plugin (WASM)

Build:

```bash
cargo build --release --target wasm32-unknown-unknown --manifest-path plugins/finance/Cargo.toml
```

Use from a comparator spec:

```yaml
fee: { comparator: plugin, plugin: finance_money }
```

and declare it in the run spec so every verification engine (in-process or remote worker) registers it before replay:

```toml
[run]
# ... other run keys ...
comparator_plugins = [
  { name = "finance_money", path = "../../plugins/finance/target/wasm32-unknown-unknown/release/amap_plugin_finance.wasm" },
]
```

The `[[run.comparator_plugins]]` array-of-tables form also works, but it must come after every other `[run]` key, otherwise TOML assigns those keys to the plugin entry (which is rejected as an unknown field). Paths are relative to the run spec and must stay inside `AMAP_WORKER_ROOT`. A spec that references an undeclared plugin fails at engine build time, before any scenario runs.

For ad-hoc comparisons the CLI accepts the same plugin directly:
`amap compare --plugin finance_money=plugins/finance/target/wasm32-unknown-unknown/release/amap_plugin_finance.wasm ...`.
