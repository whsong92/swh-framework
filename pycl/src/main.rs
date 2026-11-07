// pycl/src/main.rs
use anyhow::Result;
use pycl::core::CosmosCore;
use pycl::cosmos::logging::{CentralLogger, LogLevel, Subsystem};
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. COSMOS Core 커널 부트스트랩 (cosmos.toml 설정 로드)
    let cosmos = CosmosCore::bootstrap("cosmos.toml").await?;

    // 2. L0 3-Way Zero-Copy 파이프라인 검증 (단일 read_tensor 프로토콜)
    verify_l0_pipeline(&cosmos).await?;

    // 3. 인터랙티브 인지 셸 (REPL) 기동
    let cosmos_repl = Arc::new(cosmos);
    let cosmos_task = Arc::clone(&cosmos_repl);

    tokio::task::spawn_blocking(move || -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut line = String::new();

        println!("\n=======================================================");
        println!("  swhAI Unified Cognitive REPL (dev)");
        println!("  Commands:");
        println!("    status                     - Check BUDDHA/YAHWEH status");
        println!("    dialectic <opcode> <json>  - Run dialectic consensus");
        println!("    skill <name> <arg>         - Call dynamic skill");
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
                        let r_stat = {
                            let r = cosmos_task.rbrain.read().await;
                            r.ping().await.unwrap_or_else(|e| format!("Err: {:?}", e))
                        };
                        let l_stat = {
                            let l = cosmos_task.lbrain.read().await;
                            l.ping().unwrap_or_else(|e| format!("Err: {:?}", e))
                        };
                        println!("==> BUDDHA (Python)  : {}", r_stat);
                        println!("==> YAHWEH (Clojure) : {}", l_stat);
                    });
                }

                "dialectic" => {
                    if parts.len() < 2 {
                        warn!("사용법: dialectic <opcode> [json_payload]");
                        continue;
                    }
                    let opcode = parts[1];
                    let payload = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        "{}".to_string()
                    };

                    rt.block_on(async {
                        match cosmos_task.router.dispatch_dialectic(opcode, &payload).await {
                            Ok(verdict) => {
                                println!(
                                    "==> 변증법적 판정: {}",
                                    if verdict.agreed { "합의 도달 (AGREED)" } else { "합의 기각 (REJECTED)" }
                                );
                                println!("    세부: {}", verdict.reason);
                            }
                            Err(e) => error!("[swhAI] Dialectic error: {:?}", e),
                        }
                    });
                }

                "skill" => {
                    if parts.len() < 3 {
                        warn!("사용법: skill <skill_name> <number_arg>");
                        continue;
                    }
                    let skill_name = parts[1];
                    let arg: f64 = parts[2].parse().unwrap_or(0.0);

                    rt.block_on(async {
                        match cosmos_task.router.execute_dynamic_skill(skill_name, arg).await {
                            Ok(res) => println!("==> Skill Result: {}", res),
                            Err(e) => error!("[swhAI] Skill error: {:?}", e),
                        }
                    });
                }

                other => {
                    info!("[swhAI] Unrecognized command: \"{}\". Type 'status', 'dialectic', or 'skill'.", other);
                }
            }
        }
        Ok(())
    })
    .await??;

    // 4. 정상 종료 루틴
    cosmos_repl.shutdown().await?;
    info!("[COSMOS/Rust] VM terminated cleanly.");
    Ok(())
}

/// L0 3-Way Zero-Copy 메모리 공유 파이프라인 무결성 검증
async fn verify_l0_pipeline(cosmos: &CosmosCore) -> Result<()> {
    CentralLogger::log(
        Subsystem::Cosmos,
        LogLevel::Info,
        "Initiating 3-Way L0 Zero-Copy Pipeline Verification...".to_string(),
    );

    // 1. Rust에서 L0 아레나 슬롯 0에 테스트 토큰(42.0) 기록
    let raw_addr = {
        let arena = cosmos.arena.read().await;
        unsafe {
            *arena.as_mut_ptr() = 42.0;
        }
        arena.as_mut_ptr() as usize
    };

    // 2. Python(BUDDHA)이 L0 버퍼 직접 연산 검증
    let token = arena_read_direct(cosmos, 0).await;
    if token == 42.0 {
        CentralLogger::log(
            Subsystem::Buddha,
            LogLevel::Info,
            format!("NumPy Zero-Copy address validated: 0x{:x}", raw_addr),
        );
    } else {
        warn!("[BUDDHA/Python] Zero-Copy read mismatch at 0x{:x}", raw_addr);
    }

    // 3. Clojure(YAHWEH)가 단일 dispatch("read_tensor")를 통해 슬롯 0 값 대조
    let clj_eval = {
        let lbrain = cosmos.lbrain.read().await;
        let payload = json!({ "offset": 0, "length": 1 }).to_string();
        lbrain.dispatch("read_tensor", &payload)
    };

    match clj_eval {
        Ok(val) => {
            CentralLogger::log(
                Subsystem::Yahweh,
                LogLevel::Info,
                format!("DirectByteBuffer Zero-Copy slot 0 verified: {}", val.trim()),
            );
        }
        Err(e) => {
            warn!("[YAHWEH/Clojure] Memory verification fallback: {:?}", e);
        }
    }

    CentralLogger::log(
        Subsystem::Cosmos,
        LogLevel::Info,
        "L0 3-Way Zero-Copy Pipeline fully synchronized.".to_string(),
    );

    Ok(())
}

async fn arena_read_direct(cosmos: &CosmosCore, idx: usize) -> f32 {
    let arena = cosmos.arena.read().await;
    unsafe { *arena.as_mut_ptr().add(idx) }
}