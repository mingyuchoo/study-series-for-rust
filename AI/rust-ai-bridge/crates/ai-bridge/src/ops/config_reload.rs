//! 설정 핫 리로드 — 원자적 적용과 **롤백 캐스케이드**.
//!
//! 리로드는 순서가 있고, 뒤 단계가 실패하면 **앞 단계를 되돌립니다.** 반쯤
//! 적용된 설정으로 게이트웨이가 도는 상태는 만들지 않습니다.
//!
//! ```text
//! 1. policy    실패 → 즉시 중단 (되돌릴 것이 없음)
//! 2. inventory 실패 → policy 롤백
//! 3. catalog   실패 → inventory 롤백 + policy 롤백 + 카탈로그 파일 복원
//! 4. principal 실패 → catalog 복원 + inventory 롤백 + policy 롤백
//! ```

use super::Service;
use crate::auth;
use anyhow::{Result,
             bail};
use std::path::Path;

/// 한 단계의 결과.
#[derive(Debug, Clone)]
pub struct ReloadStepResult {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// 번들 리로드 결과.
#[derive(Debug, Clone, Default)]
pub struct ReloadBundleResult {
    pub steps: Vec<ReloadStepResult>,
    /// 되돌렸음 — **적용되지 않았습니다.**
    pub rolled_back: bool,
}

impl ReloadBundleResult {
    fn step(&mut self, name: &str, ok: bool, message: impl Into<String>) {
        self.steps.push(ReloadStepResult {
            name: name.to_string(),
            ok,
            message: message.into(),
        });
    }
}

impl Service {
    pub fn policy_path(&self) -> Option<&Path> { self.d.policy_path.as_deref() }

    pub fn systems_path(&self) -> Option<&Path> { self.d.systems_path.as_deref() }

    pub fn principal_file_path(&self) -> Option<&Path> { self.d.principal_path.as_deref() }

    pub fn tools_catalog_path(&self) -> Option<&Path> { self.d.catalog.as_ref().and_then(|c| c.path()) }

    pub fn reload_stamp_path(&self) -> Option<&Path> { self.d.reload_stamp_path.as_deref() }

    pub fn read_config_file(&self, path: Option<&Path>) -> String { path.and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default() }

    /// 인벤토리가 바뀐 뒤 어댑터를 재배선하고 MCP 도구 목록을 맞춥니다.
    async fn after_inventory_change(&self) -> Result<()> {
        let (Some(factory), Some(reg)) = (&self.d.adapter_factory, &self.d.registry) else {
            return Ok(());
        };
        let mut opts = self.d.systems_options.clone();
        opts.inventory = self.d.inventory.clone();
        factory.rebind(reg, &opts).await?;

        if let Some(c) = &self.d.catalog
            && c.enabled()
        {
            c.reload()?;
        }
        Ok(())
    }

    /// 정책 YAML 을 적용합니다. **실패하면 파일을 되돌립니다.**
    pub async fn apply_policy_yaml(&self, actor: &str, body: &str) -> Result<()> {
        let (Some(path), Some(engine)) = (self.d.policy_path.clone(), self.d.policy.clone()) else {
            bail!("ops: 정책 경로가 없습니다");
        };
        let (Some(reg), Some(inv)) = (self.d.registry.clone(), self.d.inventory.clone()) else {
            bail!("ops: 레지스트리·인벤토리가 필요합니다");
        };

        let _guard = self.config_lock.lock().await;
        let backup = std::fs::read(&path).ok();

        auth::atomic_write_public(&path, body.as_bytes(), 0o644)?;
        match engine.reload(&path, &reg, &inv) {
            | Ok(_) => {
                self.audit_op(actor, "policy.apply", "allowed", "정책 Apply", "").await;
                self.touch_reload_stamp();
                Ok(())
            },
            | Err(e) => {
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(&path, &b, 0o644);
                }
                self.audit_op(actor, "policy.apply", "denied", "정책 Apply", &e.to_string()).await;
                Err(e)
            },
        }
    }

    /// 인벤토리 YAML 을 적용하고 어댑터를 재배선합니다.
    pub async fn apply_inventory_yaml(&self, actor: &str, body: &str) -> Result<()> {
        let (Some(path), Some(inv)) = (self.d.systems_path.clone(), self.d.inventory.clone()) else {
            bail!("ops: 인벤토리 경로가 없습니다");
        };

        let _guard = self.config_lock.lock().await;
        let backup = std::fs::read(&path).ok();

        auth::atomic_write_public(&path, body.as_bytes(), 0o644)?;

        let result = async {
            inv.reload(&path)?;
            self.after_inventory_change().await
        }
        .await;

        match result {
            | Ok(()) => {
                self.audit_op(actor, "inventory.apply", "allowed", "인벤토리 Apply", "").await;
                self.touch_reload_stamp();
                Ok(())
            },
            | Err(e) => {
                // 인벤토리를 되돌리고 어댑터도 원래대로 재배선합니다.
                let _ = inv.rollback();
                let _ = self.after_inventory_change().await;
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(&path, &b, 0o644);
                }
                self.audit_op(actor, "inventory.apply", "denied", "인벤토리 Apply", &e.to_string()).await;
                Err(e)
            },
        }
    }

    /// 동적 도구 카탈로그를 적용합니다.
    pub async fn apply_tools_catalog_yaml(&self, actor: &str, body: &str) -> Result<(usize, usize)> {
        let Some(catalog) = self.d.catalog.clone() else {
            bail!("ops: 도구 카탈로그가 설정되지 않았습니다");
        };
        let Some(path) = catalog.path().map(|p| p.to_path_buf()) else {
            bail!("ops: 도구 카탈로그 경로가 없습니다");
        };

        let _guard = self.config_lock.lock().await;
        let backup = std::fs::read(&path).ok();

        auth::atomic_write_public(&path, body.as_bytes(), 0o644)?;

        match catalog.reload() {
            | Ok((added, removed)) => {
                self.audit_op(
                    actor,
                    "toolcatalog.apply",
                    "allowed",
                    &format!("도구 카탈로그 Apply added={added} removed={removed}"),
                    "",
                )
                .await;
                self.touch_reload_stamp();
                Ok((added, removed))
            },
            | Err(e) => {
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(&path, &b, 0o644);
                    // 이전 카탈로그를 다시 적용합니다 (best-effort).
                    let _ = catalog.reload();
                }
                self.audit_op(actor, "toolcatalog.apply", "denied", "도구 카탈로그 Apply", &e.to_string()).await;
                Err(e)
            },
        }
    }

    /// 주체 YAML 을 적용합니다 (HTTP 모드 전용).
    pub async fn apply_principal_yaml(&self, actor: &str, body: &str) -> Result<()> {
        let (Some(path), Some(_)) = (self.d.principal_path.clone(), self.d.tokens.clone()) else {
            bail!("ops: 주체 핫 리로드를 쓸 수 없습니다(stdio 이거나 경로 없음)");
        };

        let _guard = self.config_lock.lock().await;
        let backup = std::fs::read(&path).ok();

        auth::atomic_write_public(&path, body.as_bytes(), 0o600)?;

        match self.reload_tokens_pub(&path) {
            | Ok(()) => {
                self.audit_op(actor, "principal.apply", "allowed", "주체 Apply", "").await;
                self.touch_reload_stamp();
                Ok(())
            },
            | Err(e) => {
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(&path, &b, 0o600);
                    let _ = self.reload_tokens_pub(&path);
                }
                self.audit_op(actor, "principal.apply", "denied", "주체 Apply", &e.to_string()).await;
                Err(e)
            },
        }
    }

    pub(crate) fn reload_tokens_pub(&self, path: &Path) -> Result<()> {
        let Some(tokens) = &self.d.tokens else {
            return Ok(());
        };
        let reg = self.d.registry.clone();
        let inv = self.d.inventory.clone();
        let validate = move |ids: &[crate::auth::Identity]| -> Result<()> {
            if let (Some(reg), Some(inv)) = (&reg, &inv) {
                crate::policy::validate_allowlists(ids, reg, inv)?;
            }
            Ok(())
        };
        tokens.reload(path, Some(&validate))?;
        Ok(())
    }

    /// SIGHUP 등가 — **순서대로 리로드하고 실패하면 되돌립니다.**
    pub async fn reload_config_bundle(&self, actor: &str) -> ReloadBundleResult {
        let _guard = self.config_lock.lock().await;
        let mut out = ReloadBundleResult::default();

        // 카탈로그 파일 내용을 미리 잡아둡니다 (롤백용).
        let catalog_path = self.tools_catalog_path().map(|p| p.to_path_buf());
        let catalog_body = catalog_path.as_ref().and_then(|p| std::fs::read(p).ok());

        // --- 1. 정책 ---
        let policy_ok = match (&self.d.policy, &self.d.policy_path, &self.d.registry, &self.d.inventory) {
            | (Some(e), Some(p), Some(r), Some(i)) => match e.reload(p, r, i) {
                | Ok((old, new)) => {
                    out.step("policy", true, format!("{old} → {new}"));
                    true
                },
                | Err(err) => {
                    out.step("policy", false, err.to_string());
                    false
                },
            },
            | _ => {
                out.step("policy", false, "정책 경로·레지스트리가 없습니다");
                false
            },
        };
        if !policy_ok {
            // 되돌릴 것이 없습니다 — 활성 정책은 그대로입니다.
            self.finish_bundle(actor, &mut out).await;
            return out;
        }

        // --- 2. 인벤토리 ---
        let inventory_ok = match (&self.d.inventory, &self.d.systems_path) {
            | (Some(inv), Some(p)) => {
                let r = inv.reload(p).and(Ok(()));
                match r {
                    | Ok(()) => match self.after_inventory_change().await {
                        | Ok(()) => {
                            out.step("inventory", true, format!("{} 시스템", inv.len()));
                            true
                        },
                        | Err(e) => {
                            let _ = inv.rollback();
                            let _ = self.after_inventory_change().await;
                            out.step("inventory", false, e.to_string());
                            false
                        },
                    },
                    | Err(e) => {
                        out.step("inventory", false, e.to_string());
                        false
                    },
                }
            },
            | _ => {
                out.step("inventory", true, "건너뜀(경로 없음)");
                true
            },
        };
        if !inventory_ok {
            if let Some(e) = &self.d.policy
                && e.rollback().is_ok()
            {
                out.step("rollback.policy", true, "정책을 되돌렸습니다");
            }
            out.rolled_back = true;
            self.finish_bundle(actor, &mut out).await;
            return out;
        }

        // --- 3. 도구 카탈로그 ---
        let catalog_ok = match &self.d.catalog {
            | Some(c) if c.enabled() => match c.reload() {
                | Ok((a, r)) => {
                    out.step("toolcatalog", true, format!("added={a} removed={r}"));
                    true
                },
                | Err(e) => {
                    out.step("toolcatalog", false, e.to_string());
                    false
                },
            },
            | _ => {
                out.step("toolcatalog", true, "건너뜀(비활성)");
                true
            },
        };
        if !catalog_ok {
            self.rollback_all(&catalog_path, &catalog_body).await;
            out.step("rollback", true, "policy+inventory(+catalog file)");
            out.rolled_back = true;
            self.finish_bundle(actor, &mut out).await;
            return out;
        }

        // --- 4. 주체 (HTTP 모드에서만) ---
        if self.d.tokens.is_some()
            && let Some(p) = self.d.principal_path.clone()
        {
            match self.reload_tokens_pub(&p) {
                | Ok(()) => {
                    out.step("principal", true, "주체 디렉터리 리로드");
                },
                | Err(e) => {
                    out.step("principal", false, e.to_string());
                    self.rollback_all(&catalog_path, &catalog_body).await;
                    out.step("rollback", true, "policy+inventory+catalog");
                    out.rolled_back = true;
                },
            }
        }

        self.finish_bundle(actor, &mut out).await;
        out
    }

    async fn rollback_all(&self, catalog_path: &Option<std::path::PathBuf>, catalog_body: &Option<Vec<u8>>) {
        if let (Some(p), Some(b), Some(c)) = (catalog_path, catalog_body, &self.d.catalog) {
            let _ = auth::atomic_write_public(p, b, 0o644);
            let _ = c.reload();
        }
        if let Some(inv) = &self.d.inventory
            && inv.rollback().is_ok()
        {
            let _ = self.after_inventory_change().await;
        }
        if let Some(e) = &self.d.policy {
            let _ = e.rollback();
        }
    }

    async fn finish_bundle(&self, actor: &str, out: &mut ReloadBundleResult) {
        let ok_steps = out.steps.iter().filter(|s| s.ok && !s.name.starts_with("rollback")).count();
        let decision = if ok_steps == 0 || out.rolled_back { "denied" } else { "allowed" };
        self.audit_op(
            actor,
            "config.reload",
            decision,
            &format!("config bundle reload ok_steps={ok_steps} rolled_back={}", out.rolled_back),
            "",
        )
        .await;

        // 되돌린 경우에는 다른 인스턴스를 깨우지 않습니다.
        if !out.rolled_back {
            self.touch_reload_stamp();
        }
    }

    /// 다른 인스턴스가 설정 변경을 알아채도록 stamp 파일을 건드립니다.
    pub(crate) fn touch_reload_stamp(&self) {
        let Some(p) = &self.d.reload_stamp_path else {
            return;
        };
        let body = format!("{}\n", crate::clock::to_rfc3339_nanos(chrono::Utc::now()));
        let _ = auth::atomic_write_public(p, body.as_bytes(), 0o644);
    }

    /// 클러스터에 리로드를 알립니다.
    pub async fn notify_cluster_reload(&self, actor: &str) -> Result<()> {
        if self.d.reload_stamp_path.is_none() {
            bail!("ops: reload stamp 경로가 설정되지 않았습니다(-reload-stamp)");
        }
        self.touch_reload_stamp();
        self.audit_op(actor, "config.reload_stamp", "allowed", "reload stamp touched", "").await;
        Ok(())
    }
}
