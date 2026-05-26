// Single test binary — includes all test modules
#[path = "suite/compaction_pair_test.rs"]
mod compaction_pair_test;
#[path = "suite/file_snapshot_test.rs"]
mod file_snapshot_test;
#[path = "suite/fork_test.rs"]
mod fork_test;
#[path = "suite/image_token_cap_test.rs"]
mod image_token_cap_test;
#[path = "suite/ingestion_test.rs"]
mod ingestion_test;
#[path = "suite/smart_compact_test.rs"]
mod smart_compact_test;
#[path = "suite/system_prompt_agent_test.rs"]
mod system_prompt_agent_test;
#[path = "suite/system_prompt_test.rs"]
mod system_prompt_test;
#[path = "suite/token_counter_test.rs"]
mod token_counter_test;
#[path = "suite/turn_store_test.rs"]
mod turn_store_test;
