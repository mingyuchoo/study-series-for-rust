//! 에이전트 토큰 발급·해시.
//!
//! **평문 토큰은 stdout 에 한 번만 나옵니다.** 어떤 파일에도 쓰지 않습니다 —
//! `principal.yaml` 에는 `token_sha256` 만 들어갑니다. 평문을 파일이나 git 에
//! 두면 그 순간 에이전트를 누구나 사칭할 수 있습니다.

use ai_bridge::auth;
use anyhow::{Result,
             bail};
use clap::{Parser,
           Subcommand};

#[derive(Parser)]
#[command(name = "agentctl", version, about = "에이전트 토큰 발급·해시")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 새 토큰을 발급합니다. **평문은 다시 볼 수 없습니다.**
    Token {
        /// principal.yaml 에 붙일 스니펫에 넣을 user_id (표시용).
        #[arg(long, default_value = "")]
        user: String,
    },
    /// 이미 발급된 토큰의 해시를 다시 구합니다.
    ///
    /// 새 토큰을 발급하지 않고 `principal.yaml` 항목을 검증할 때 씁니다.
    Hash { token: String },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        | Cmd::Token {
            user,
        } => {
            let token = auth::generate_token();
            let hash = auth::token_hash(&token);

            println!("# 새 에이전트 토큰이 발급되었습니다. 이 평문 토큰은 다시 볼 수 없습니다.");
            println!("# 에이전트에만 안전하게 전달하세요:");
            println!();
            println!("AGENT_TOKEN={token}");
            println!();
            println!("# principal.yaml 의 해당 에이전트 항목에 아래를 넣으세요(평문은 넣지 마세요):");
            if user.is_empty() {
                println!("#   - user_id: <에이전트 id>");
            } else {
                println!("#   - user_id: {user}");
            }
            println!("#     kind: agent");
            println!("#     roles: [...]");
            println!("#     allowed_tools: [...]      # 역할 권한 안에서 더 좁힙니다");
            println!("#     allowed_systems: [...]");
            println!("      token_sha256: {hash}");
            Ok(())
        },
        | Cmd::Hash {
            token,
        } => {
            if token.is_empty() {
                bail!("사용법: agentctl hash <token>");
            }
            println!("{}", auth::token_hash(&token));
            Ok(())
        },
    }
}
