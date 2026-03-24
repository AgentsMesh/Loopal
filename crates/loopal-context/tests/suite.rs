// Single test binary — includes all test modules
#[path = "suite/compaction_pair_test.rs"]
mod compaction_pair_test;
#[path = "suite/compaction_test.rs"]
mod compaction_test;
#[path = "suite/degradation_test.rs"]
mod degradation_test;
#[path = "suite/ingestion_test.rs"]
mod ingestion_test;
#[path = "suite/pipeline_test.rs"]
mod pipeline_test;
#[path = "suite/smart_compact_test.rs"]
mod smart_compact_test;
#[path = "suite/store_test.rs"]
mod store_test;
#[path = "suite/system_prompt_test.rs"]
mod system_prompt_test;
#[path = "suite/token_counter_test.rs"]
mod token_counter_test;
