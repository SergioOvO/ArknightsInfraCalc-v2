//! 会客室静态求值；概率倾向、线索事件和交流状态不在本域计算。

use crate::error::{Error, Result};
use crate::support_facility::{
    evaluate_support_room, SupportFacility, SupportRegistry, SupportRoomInput, SupportRoomResult,
};

pub fn evaluate_meeting(
    input: &SupportRoomInput,
    registry: &SupportRegistry,
) -> Result<SupportRoomResult> {
    if input.facility != SupportFacility::Meeting {
        return Err(Error::msg("meeting evaluator requires meeting input"));
    }
    evaluate_support_room(input, registry)
}
