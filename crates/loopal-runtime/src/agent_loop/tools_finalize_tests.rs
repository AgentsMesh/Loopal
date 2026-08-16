use loopal_provider_api::ContentBlock;

use super::collect_item_updates;

#[test]
fn non_tool_result_blocks_do_not_update_batch_items() {
    let blocks = vec![(
        0,
        ContentBlock::Text {
            text: "observer note".into(),
        },
    )];
    assert!(collect_item_updates(&blocks, vec!["tool-1".into()]).is_empty());
}
