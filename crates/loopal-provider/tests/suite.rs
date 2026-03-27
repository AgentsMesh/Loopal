// Single test binary — includes all test modules
#[path = "suite/anthropic_request_test.rs"]
mod anthropic_request_test;
#[path = "suite/anthropic_stream_edge_test.rs"]
mod anthropic_stream_edge_test;
#[path = "suite/anthropic_stream_test.rs"]
mod anthropic_stream_test;
#[path = "suite/google_request_test.rs"]
mod google_request_test;
#[path = "suite/google_stream_test.rs"]
mod google_stream_test;
#[path = "suite/model_info_test.rs"]
mod model_info_test;
#[path = "suite/openai_request_test.rs"]
mod openai_request_test;
#[path = "suite/openai_stream_edge_test.rs"]
mod openai_stream_edge_test;
#[path = "suite/openai_stream_test.rs"]
mod openai_stream_test;
#[path = "suite/router_test.rs"]
mod router_test;
#[path = "suite/stream_anthropic_edge_test.rs"]
mod stream_anthropic_edge_test;
#[path = "suite/stream_anthropic_test.rs"]
mod stream_anthropic_test;
#[path = "suite/stream_google_test.rs"]
mod stream_google_test;
#[path = "suite/stream_helpers.rs"]
mod stream_helpers;
#[path = "suite/stream_openai_edge_test.rs"]
mod stream_openai_edge_test;
#[path = "suite/stream_openai_test.rs"]
mod stream_openai_test;
