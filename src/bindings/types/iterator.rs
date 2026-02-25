use crate::bindings::tarantool::tarantool::types::{BoxError, IteratorType};

use tarantool::error::TarantoolErrorCode;

pub fn convert_iterator_type(t: IteratorType) -> Result<tarantool::index::IteratorType, BoxError> {
    use tarantool::index::IteratorType as Native;

    Ok(match t {
        IteratorType::IterEq => Native::Eq,
        IteratorType::IterReq => Native::Req,
        IteratorType::IterAll => Native::All,
        IteratorType::IterLt => Native::LT,
        IteratorType::IterLe => Native::LE,
        IteratorType::IterGe => Native::GE,
        IteratorType::IterGt => Native::GT,
        IteratorType::IterBitsAllSet => Native::BitsAllSet,
        IteratorType::IterBitsAnySet => Native::BitsAnySet,
        IteratorType::IterBitsAllNotSet => Native::BitsAllNotSet,
        IteratorType::IterOverlaps => Native::Overlaps,
        IteratorType::IterNeighbor => Native::Neighbor,
        // need for patch https://github.com/picodata/tarantool-module/blob/e4c78b9b0e59ef45ab6e8a7023aa192b28e330c0/tarantool/src/index.rs#L57
        // https://github.com/tarantool/tarantool/pull/10028
        IteratorType::IterNp | IteratorType::IterPp => {
            return Err(BoxError::new(
                TarantoolErrorCode::IteratorType as u32,
                "IteratorType",
            ))
        }
    })
}
