// pycl/src/main.rs
use anyhow::Result;
use pycl::core::CosmosCore;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. COSMOS Core 커널 부트스트랩 (순수 런타임 기동)
    let cosmos = CosmosCore::bootstrap("cosmos.toml").await?;

    // 2. 인터랙티브 인지 셸 (REPL) 기동
    let cosmos_repl = Arc::new(cosmos);
    let cosmos_task = Arc::clone(&cosmos_repl);

    tokio::task::spawn_blocking(move || -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut line = String::new();

        println!("\n=======================================================");
        println!("  swhAI Unified Kernel REPL (dev)");
        println!("  Commands:");
        println!("    status                     - Check BUDDHA & YAHWEH status");
        println!("    buddha <opcode> [json]     - Direct dispatch to BUDDHA");
        println!("    yahweh <opcode> [json]     - Direct dispatch to YAHWEH");
        println!("    broadcast <opcode> [json]  - Broadcast to both engines");
        println!("    quit / exit                - Shutdown");
        println!("=======================================================\n");

        let rt = tokio::runtime::Handle::current();

        loop {
            print!("swhAI> ");
            stdout.flush()?;
            line.clear();

            if reader.read_line(&mut line)? == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            match parts[0] {
                "quit" | "exit" => {
                    info!("[COSMOS/Rust] Shutdown initiated by user.");
                    break;
                }

                "status" => {
                    rt.block_on(async {
                        let (r_stat, l_stat) = cosmos_task.router.ping_all().await;
                        println!("==> BUDDHA (Python)  : {}", r_stat.unwrap_or_else(|e| format!("Err: {:?}", e)));
                        println!("==> YAHWEH (Clojure) : {}", l_stat.unwrap_or_else(|e| format!("Err: {:?}", e)));
                    });
                }

                "buddha" => {
                    if parts.len() < 2 {
                        warn!("Usage: buddha <opcode> [json_payload]");
                        continue;
                    }
                    let opcode = parts[1];
                    let payload = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        "{}".to_string()
                    };

                    rt.block_on(async {
                        match cosmos_task.router.dispatch_buddha(opcode, &payload).await {
                            Ok(res) => println!("==> BUDDHA Result: {}", res),
                            Err(e) => error!("[swhAI] BUDDHA error: {:?}", e),
                        }
                    });
                }

                "yahweh" => {
                    if parts.len() < 2 {
                        warn!("Usage: yahweh <opcode> [json_payload]");
                        continue;
                    }
                    let opcode = parts[1];
                    let payload = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        "{}".to_string()
                    };

                    rt.block_on(async {
                        match cosmos_task.router.dispatch_yahweh_async(opcode, &payload).await {
                            Ok(res) => println!("==> YAHWEH Result: {}", res),
                            Err(e) => error!("[swhAI] YAHWEH error: {:?}", e),
                        }
                    });
                }

                "broadcast" => {
                    if parts.len() < 2 {
                        warn!("Usage: broadcast <opcode> [json_payload]");
                        continue;
                    }
                    let opcode = parts[1];
                    let payload = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        "{}".to_string()
                    };

                    rt.block_on(async {
                        let (b_res, y_res) = cosmos_task.router.broadcast(opcode, &payload).await;
                        println!("==> BUDDHA    : {}", b_res.unwrap_or_else(|e| format!("Err: {:?}", e)));
                        println!("==> YAHWEH    : {}", y_res.unwrap_or_else(|e| format!("Err: {:?}", e)));
                    });
                }

                other => {
                    info!(
                        "[swhAI] Unrecognized command: \"{}\". Type 'status', 'buddha', 'yahweh', 'broadcast', or 'quit'.",
                        other
                    );
                }
            }
        }
        Ok(())
    })
    .await??;

    cosmos_repl.shutdown().await?;
    info!("[COSMOS/Rust] VM terminated cleanly.");
    Ok(())
}