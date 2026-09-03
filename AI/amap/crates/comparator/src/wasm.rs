//! WebAssembly comparator plugins sandboxed by Wasmtime (design §8).
//!
//! Plugin ABI (core wasm, no WASI required):
//! ```text
//! (memory (export "memory"))
//! (func (export "alloc") (param i32) (result i32))
//! (func (export "compare") (param i32 i32 i32 i32) (result i32))  ; expected ptr/len, actual ptr/len
//!      ; returns 0 = equal, 1 = different, <0 = error
//! (func (export "message_ptr") (result i32))  ; optional: last message
//! (func (export "message_len") (result i32))
//! ```
//! Inputs are UTF-8 JSON. Execution is metered with fuel and memory-isolated.
use crate::{Comparator, ComparatorError, ComparisonContext};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;
use wasmtime::{Config, Engine, Instance, Module, Store, TypedFunc};

pub struct WasmComparator {
    name: String,
    engine: Engine,
    module: Module,
    fuel: u64,
    /// Serialise calls; a fresh Store per call gives deterministic, isolated execution.
    lock: Mutex<()>,
}

impl WasmComparator {
    pub fn from_file(name: impl Into<String>, path: impl AsRef<Path>) -> Result<Self, ComparatorError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(name, &bytes)
    }

    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Result<Self, ComparatorError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        let module = Module::new(&engine, bytes).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        Ok(Self { name: name.into(), engine, module, fuel: 50_000_000, lock: Mutex::new(()) })
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    fn call(&self, expected: &Value, actual: &Value) -> Result<(i32, String), ComparatorError> {
        let _guard = self.lock.lock().unwrap();
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(self.fuel).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        let instance = Instance::new(&mut store, &self.module, &[]).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| ComparatorError::Plugin("no exported memory".into()))?;
        let alloc: TypedFunc<i32, i32> = instance.get_typed_func(&mut store, "alloc").map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        let compare: TypedFunc<(i32, i32, i32, i32), i32> =
            instance.get_typed_func(&mut store, "compare").map_err(|e| ComparatorError::Plugin(e.to_string()))?;

        let e = serde_json::to_vec(expected).unwrap();
        let a = serde_json::to_vec(actual).unwrap();
        let ep = alloc.call(&mut store, e.len() as i32).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        memory.write(&mut store, ep as usize, &e).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        let ap = alloc.call(&mut store, a.len() as i32).map_err(|e| ComparatorError::Plugin(e.to_string()))?;
        memory.write(&mut store, ap as usize, &a).map_err(|e| ComparatorError::Plugin(e.to_string()))?;

        let code = compare
            .call(&mut store, (ep, e.len() as i32, ap, a.len() as i32))
            .map_err(|e| ComparatorError::Plugin(format!("trap or out of fuel: {e}")))?;

        let mut message = String::new();
        if let (Ok(mp), Ok(ml)) = (
            instance.get_typed_func::<(), i32>(&mut store, "message_ptr"),
            instance.get_typed_func::<(), i32>(&mut store, "message_len"),
        ) {
            let p = mp.call(&mut store, ()).unwrap_or(0) as usize;
            let l = ml.call(&mut store, ()).unwrap_or(0) as usize;
            if l > 0 {
                let mut buf = vec![0u8; l];
                if memory.read(&store, p, &mut buf).is_ok() {
                    message = String::from_utf8_lossy(&buf).to_string();
                }
            }
        }
        Ok((code, message))
    }
}

impl Comparator for WasmComparator {
    fn name(&self) -> &str {
        &self.name
    }
    fn compare(&self, expected: &Value, actual: &Value, _: &ComparisonContext<'_>) -> Option<String> {
        match self.call(expected, actual) {
            Ok((0, _)) => None,
            Ok((1, msg)) => Some(if msg.is_empty() { "plugin reported difference".into() } else { msg }),
            Ok((code, msg)) => Some(format!("plugin error {code}: {msg}")),
            Err(e) => Some(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::FieldRule;
    use std::collections::HashSet;

    /// Minimal WAT plugin: equal iff both payloads have the same byte length.
    const WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func (export "compare") (param i32 i32 i32 i32) (result i32)
    (if (result i32) (i32.eq (local.get 1) (local.get 3)) (then (i32.const 0)) (else (i32.const 1)))))
"#;

    #[test]
    fn runs_wat_plugin() {
        let cmp = WasmComparator::from_bytes("len", WAT.as_bytes()).unwrap();
        let seen = Mutex::new(HashSet::new());
        let rule = FieldRule::exact();
        let ctx = ComparisonContext { path: "x", rule: &rule, seen_unique: &seen };
        assert!(cmp.compare(&serde_json::json!(12), &serde_json::json!(34), &ctx).is_none());
        assert!(cmp.compare(&serde_json::json!(12), &serde_json::json!(345), &ctx).is_some());
    }
}
