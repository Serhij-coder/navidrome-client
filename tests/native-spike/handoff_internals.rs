use super::{ControlFailure, Error, HandoffFailure, control_failure};

#[test]
fn preparation_deadlines_have_one_classification() {
    assert!(matches!(
        control_failure(ControlFailure::Deadline),
        HandoffFailure::Deadline
    ));
    assert!(matches!(
        control_failure(ControlFailure::Engine(Error::Deadline)),
        HandoffFailure::Deadline
    ));
    assert!(matches!(
        control_failure(ControlFailure::Engine(Error::Protocol)),
        HandoffFailure::Engine(Error::Protocol)
    ));
    assert!(matches!(
        control_failure(ControlFailure::Cancelled),
        HandoffFailure::Cancelled
    ));
    assert!(matches!(
        control_failure(ControlFailure::Stopped),
        HandoffFailure::Stopped
    ));
}
