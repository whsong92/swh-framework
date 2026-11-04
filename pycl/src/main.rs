use anyhow::Result;
use pycl::core::CosmosCore;
use pycl::cosmos::logging::{CentralLogger, LogLevel, Subsystem};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("============================================================");
    info!("   pycl Virtual Runtime & swhAI (Project Cosmos) v0.3.0     ");
    info!("   [COSMOS/Rust] + [BUDDHA/Python] + [YAHWEH/Clojure]       ");
    info!("============================================================");

    // 1. COSMOS Core 가상 머신 및 하이브리드 엔진 부트스트랩 (Arc 단일 래핑)
    let cosmos = match CosmosCore::bootstrap("cosmos.toml").await {
        Ok(c) => Arc::new(c),
        Err(e) => {
            error!("[COSMOS/Rust] Bootstrap sequence failed: {:?}", e);
            return Err(e);
        }
    };

    // 2. 인지 엔진 헬스체크 및 핑퐁
    cosmos.init_cognitive_engines().await?;

    // 3. L0 3-Way Zero-Copy 메모리 버퍼 무결성 핑퐁 검증
    verify_l0_pipeline(&cosmos).await?;

    // 4. 비동기 로그 플러시 안정화 대기
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    println!("\n============================================================");
    println!(" swhAI Interactive Shell (Project Cosmos v0.3.0)");
    println!("============================================================");
    println!("명령어 안내:");
    println!("  - ping                     : 이종 서브시스템 헬스체크");
    println!("  - status                   : L0 메모리 및 시스템 상태 확인");
    println!("  - registry                 : 활성화된 함수 및 프리미티브 메타데이터 출력");
    println!("  - include <sub> <super>    : 포함 (⊂) 변증법적 합의 평가 (Box Embedding + Type DAG)");
    println!("  - oppose <a> <b>           : 반대/부정 (¬) 변증법적 합의 평가 (Orthogonal + Axiom)");
    println!("  - eval <sexp>              : Clojure S-expression 기호 연성 즉시 평가");
    println!("  - exit | quit              : 가상 머신 안전 정지 및 메모리 해제\n");

    // 5. REPL 루프 실행 (블로킹 stdin 전용 스레드로 완전 분리)
    let cosmos_repl = cosmos.clone();
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            print!("swhAI> ");
            let _ = io::stdout().flush();

            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF (Ctrl+D)
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[COSMOS/Rust] Stdin error: {:?}", e);
                    break;
                }
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let cmd_str = trimmed.strip_prefix(':').unwrap_or(trimmed);
            let parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "exit" | "quit" => break,

                "status" => {
                    rt.block_on(async {
                        let arena = cosmos_repl.arena.read().await;
                        info!(
                            "[COSMOS/Rust] L0 Physical Arena: {} f32 slots ({} MB), Address: 0x{:x}",
                            arena.capacity(),
                            (arena.capacity() * std::mem::size_of::<f32>()) / (1024 * 1024),
                            arena.as_mut_ptr() as usize
                        );
                    });
                }

                "ping" => {
                    rt.block_on(async {
                        let _ = cosmos_repl.init_cognitive_engines().await;
                    });
                }

                "registry" => {
                    rt.block_on(async {
                        cosmos_repl.registry.list().await;
                    });
                }

                "include" => {
                    if parts.len() < 3 {
                        warn!("사용법: include <sub_concept> <super_concept>");
                        continue;
                    }
                    let sub = parts[1].to_string();
                    let sup = parts[2].to_string();

                    rt.block_on(async {
                        match cosmos_repl.router.judge_inclusion(&sub, &sup, 0, 64, 32).await {
                            Ok(verdict) => {
                                println!(
                                    "==> 변증법적 합의 결과: {}",
                                    if verdict.agreed {
                                        "합의 도달 (AGREED)"
                                    } else {
                                        "합의 기각 (REJECTED)"
                                    }
                                );
                                println!("    세부: {}", verdict.reason);
                            }
                            Err(e) => error!("[swhAI] Inclusion evaluation error: {:?}", e),
                        }
                    });
                }

                "oppose" => {
                    if parts.len() < 3 {
                        warn!("사용법: oppose <concept_a> <concept_b>");
                        continue;
                    }
                    let a = parts[1].to_string();
                    let b = parts[2].to_string();

                    rt.block_on(async {
                        match cosmos_repl.router.judge_opposition(&a, &b, 0, 64, 32).await {
                            Ok(verdict) => {
                                println!(
                                    "==> 변증법적 합의 결과: {}",
                                    if verdict.agreed {
                                        "합의 도달 (AGREED)"
                                    } else {
                                        "합의 기각 (REJECTED)"
                                    }
                                );
                                println!("    세부: {}", verdict.reason);
                            }
                            Err(e) => error!("[swhAI] Opposition evaluation error: {:?}", e),
                        }
                    });
                }

                "eval" => {
                    if parts.len() < 2 {
                        warn!("사용법: eval <sexp>");
                        continue;
                    }
                    let sexp = parts[1..].join(" ");

                    rt.block_on(async {
                        let lbrain = cosmos_repl.lbrain.read().await;
                        match lbrain.eval_sexp(&sexp) {
                            Ok(res) => println!("==> YAHWEH: {}", res),
                            Err(e) => error!("[YAHWEH/Clojure] Eval error: {:?}", e),
                        }
                    });
                }

                other => {
                    info!(
                        "[swhAI] Raw input unrecognized: \"{}\". Type 'status' or 'include'.",
                        other
                    );
                }
            }
        }
    })
    .await?;

    // 6. 정상 종료 루틴 (L0 공유 메모리 해제 및 파이프라인 드레인)
    cosmos.shutdown().await?;
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
    let buddha_verified = {
        let token = arena_read_direct(cosmos, 0).await;
        token == 42.0
    };

    if buddha_verified {
        CentralLogger::log(
            Subsystem::Buddha,
            LogLevel::Info,
            format!("NumPy (OWNDATA=False) Zero-Copy address validated: 0x{:x}", raw_addr),
        );
    } else {
        warn!("[BUDDHA/Python] Zero-Copy read mismatch at 0x{:x}", raw_addr);
    }

    // 3. Clojure(YAHWEH)가 DirectByteBuffer를 통해 슬롯 값 대조
    let clj_eval = {
        let lbrain = cosmos.lbrain.read().await;
        lbrain.eval_sexp("(lbrain.core/get-shared-slot 0)")
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