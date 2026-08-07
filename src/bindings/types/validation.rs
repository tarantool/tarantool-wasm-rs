use rmpv::decode::read_value;
use std::io::Cursor;

fn read_complete_value(data: &[u8], empty_message: &str) -> Result<rmpv::Value, String> {
    if data.is_empty() {
        return Err(empty_message.to_string());
    }

    let mut cursor = Cursor::new(data);
    let value = read_value(&mut cursor).map_err(|e| format!("Invalid MsgPack data: {e}"))?;
    if cursor.position() != data.len() as u64 {
        return Err("Trailing bytes after MsgPack value".to_string());
    }
    Ok(value)
}

pub fn validate_msgpack_value(data: &[u8]) -> Result<(), String> {
    read_complete_value(data, "MessagePack data is empty").map(|_| ())
}

pub fn validate_msgpack_array(data: &[u8]) -> Result<(), String> {
    match read_complete_value(data, "Tuple data is empty")? {
        val if val.is_array() => Ok(()),
        other => Err(format!("Expected MsgPack array, got {other:?}")),
    }
}

pub fn validate_msgpack_map(data: &[u8]) -> Result<(), String> {
    match read_complete_value(data, "Map data is empty")? {
        val if val.is_map() => Ok(()),
        other => Err(format!("Expected MsgPack map, got {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_msgpack_array, validate_msgpack_map, validate_msgpack_value};

    #[test]
    fn validates_exactly_one_complete_value() {
        assert!(validate_msgpack_value(&[0x01]).is_ok());
        assert!(validate_msgpack_value(&[]).is_err());
        assert!(validate_msgpack_value(&[0xd9, 0x02, b'a']).is_err());
        assert!(validate_msgpack_value(&[0x01, 0x02]).is_err());
    }

    #[test]
    fn validates_exactly_one_complete_array() {
        assert!(validate_msgpack_array(&[0x92, 0x01, 0x02]).is_ok());
        assert!(validate_msgpack_array(&[]).is_err());
        assert!(validate_msgpack_array(&[0x92, 0x01]).is_err());
        assert!(validate_msgpack_array(&[0x81, 0xa1, b'a', 0x01]).is_err());
        assert!(validate_msgpack_array(&[0x90, 0x00]).is_err());
    }

    #[test]
    fn validates_exactly_one_complete_map() {
        assert!(validate_msgpack_map(&[0x81, 0xa1, b'a', 0x01]).is_ok());
        assert!(validate_msgpack_map(&[]).is_err());
        assert!(validate_msgpack_map(&[0x81, 0xa1, b'a']).is_err());
        assert!(validate_msgpack_map(&[0x90]).is_err());
        assert!(validate_msgpack_map(&[0x80, 0x00]).is_err());
    }
}
