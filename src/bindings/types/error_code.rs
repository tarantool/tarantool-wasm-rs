use crate::bindings::tarantool::tarantool::types::ErrorCode;

impl From<ErrorCode> for u32 {
    fn from(code: ErrorCode) -> Self {
        (code as u16) as u32
    }
}

impl core::convert::TryFrom<u32> for ErrorCode {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let v16 = u16::try_from(value).map_err(|_| ())?;
        let max = ErrorCode::TxnCommit as u16;
        if v16 <= max {
            Ok(unsafe { core::mem::transmute::<u16, ErrorCode>(v16) })
        } else {
            Err(())
        }
    }
}
