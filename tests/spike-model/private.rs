use super::*;
#[test]
fn missing_range_anchor_rebases_after_filter() {
    let mut selection = Selection::default();
    selection.select(&[ViewRow::Track(1)], 1, false, false);
    selection.select(&[ViewRow::Album(0), ViewRow::Track(4)], 4, true, false);
    assert_eq!(*selection.selected, BTreeSet::from([4]));
}
