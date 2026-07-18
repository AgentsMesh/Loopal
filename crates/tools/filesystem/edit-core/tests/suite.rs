// Single test binary — includes all test modules
#[path = "suite/diff_algorithm_test.rs"]
mod diff_algorithm_test;
#[path = "suite/hunks_test.rs"]
mod hunks_test;
#[path = "suite/multi_edit_test.rs"]
mod multi_edit_test;
#[path = "suite/no_match_hint_test.rs"]
mod no_match_hint_test;
#[path = "suite/omission_detector_test.rs"]
mod omission_detector_test;
#[path = "suite/omission_message_test.rs"]
mod omission_message_test;
#[path = "suite/patch_parser_test.rs"]
mod patch_parser_test;
#[path = "suite/search_replace_test.rs"]
mod search_replace_test;
