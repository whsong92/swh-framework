pub mod arena;
pub mod crash_guard;
pub mod logging;
pub mod storage;

// 외부(main.rs, swhai/router.rs)에서 간결하게 사용할 수 있도록 핵심 심볼 Re-export
pub use arena::{SharedArena, MEMORY_ALIGNMENT};
pub use crash_guard::{init_crash_guard, run_guarded, CrashInfo};
pub use logging::{CentralLogger, LogLevel, Subsystem};
pub use storage::{
    ConceptNode, ConceptRecord, RedisStorage, StorageHub, SurrealStorage, TopologyEdge,
    TopologyRecord,
};