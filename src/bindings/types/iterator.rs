use crate::bindings::tarantool::tarantool::types::{BoxError, IteratorType};

use tarantool::error::TarantoolErrorCode;

pub fn convert_iterator_type(t: IteratorType) -> Result<tarantool::index::IteratorType, BoxError> {
    use tarantool::index::IteratorType as Native;

    Ok(match t {
        IteratorType::Equal => Native::Eq,
        IteratorType::ReverseEqual => Native::Req,
        IteratorType::All => Native::All,
        IteratorType::LessThan => Native::LT,
        IteratorType::LessThanOrEqual => Native::LE,
        IteratorType::GreaterThanOrEqual => Native::GE,
        IteratorType::GreaterThan => Native::GT,
        IteratorType::BitsAllSet => Native::BitsAllSet,
        IteratorType::BitsAnySet => Native::BitsAnySet,
        IteratorType::BitsAllNotSet => Native::BitsAllNotSet,
        IteratorType::Overlaps => Native::Overlaps,
        IteratorType::Neighbor => Native::Neighbor,
        // need for patch https://github.com/picodata/tarantool-module/blob/e4c78b9b0e59ef45ab6e8a7023aa192b28e330c0/tarantool/src/index.rs#L57
        // https://github.com/tarantool/tarantool/pull/10028
        IteratorType::NextPrefix | IteratorType::PreviousPrefix => {
            return Err(BoxError::new(
                TarantoolErrorCode::IteratorType as u32,
                "IteratorType",
            ))
        }
    })
}
