use super::{EncodeError, encode};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::cell::Cell;

struct Many<'a>(&'a Cell<usize>);
impl Serialize for Many<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(100_000))?;
        for _ in 0..100_000 {
            self.0.set(self.0.get() + 1);
            seq.serialize_element("synthetic")?;
        }
        seq.end()
    }
}
struct Failure;
impl Serialize for Failure {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("synthetic secret"))
    }
}
#[test]
fn stops_traversal_and_does_not_expose_partial_output() {
    let count = Cell::new(0);
    let error = encode(&Many(&count), 64).unwrap_err();
    assert_eq!(error, EncodeError::TooLarge);
    assert!(count.get() < 100);
    assert!(!format!("{error:?}").contains("synthetic"));
}
#[test]
fn exact_limit_counts_escaped_output_bytes() {
    let value = "\n\"é";
    let expected = serde_json::to_vec(value).unwrap();
    assert_eq!(encode(value, expected.len()).unwrap(), expected);
    assert_eq!(
        encode(value, expected.len() - 1),
        Err(EncodeError::TooLarge)
    );
    assert_eq!(encode(&Failure, 100), Err(EncodeError::Invalid));
    assert!(!format!("{:?}", encode(&Failure, 100)).contains("secret"));
}
