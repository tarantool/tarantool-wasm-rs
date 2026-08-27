use rmpv::decode::read_value_ref;
use std::io::Cursor;

fn read_complete_value<'a>(
    data: &'a [u8],
    empty_message: &str,
) -> Result<rmpv::ValueRef<'a>, String> {
    if data.is_empty() {
        return Err(empty_message.to_string());
    }

    let mut cursor = Cursor::new(data);
    let value = read_value_ref(&mut cursor).map_err(|_| {
        let mut owning_cursor = Cursor::new(data);
        match rmpv::decode::read_value(&mut owning_cursor) {
            Err(error) => format!("Invalid MsgPack data: {error}"),
            Ok(_) => "Invalid MsgPack data: borrowed decoder rejected valid data".to_string(),
        }
    })?;
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
        rmpv::ValueRef::Array(_) => Ok(()),
        other => Err(format!(
            "Expected MsgPack array, got {:?}",
            other.to_owned()
        )),
    }
}

pub fn validate_msgpack_map(data: &[u8]) -> Result<(), String> {
    match read_complete_value(data, "Map data is empty")? {
        rmpv::ValueRef::Map(_) => Ok(()),
        other => Err(format!("Expected MsgPack map, got {:?}", other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_msgpack_array, validate_msgpack_map, validate_msgpack_value};
    use std::io::Cursor;

    fn owning_read_complete_value(data: &[u8], empty_message: &str) -> Result<rmpv::Value, String> {
        if data.is_empty() {
            return Err(empty_message.to_string());
        }
        let mut cursor = Cursor::new(data);
        let value = rmpv::decode::read_value(&mut cursor)
            .map_err(|e| format!("Invalid MsgPack data: {e}"))?;
        if cursor.position() != data.len() as u64 {
            return Err("Trailing bytes after MsgPack value".to_string());
        }
        Ok(value)
    }

    fn owning_validate_value(data: &[u8]) -> Result<(), String> {
        owning_read_complete_value(data, "MessagePack data is empty").map(|_| ())
    }

    fn owning_validate_array(data: &[u8]) -> Result<(), String> {
        match owning_read_complete_value(data, "Tuple data is empty")? {
            value if value.is_array() => Ok(()),
            other => Err(format!("Expected MsgPack array, got {other:?}")),
        }
    }

    fn owning_validate_map(data: &[u8]) -> Result<(), String> {
        match owning_read_complete_value(data, "Map data is empty")? {
            value if value.is_map() => Ok(()),
            other => Err(format!("Expected MsgPack map, got {other:?}")),
        }
    }

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

    #[test]
    fn borrowed_validation_matches_owning_decoder() {
        let mut cases = vec![
            vec![],
            vec![0xc0],
            vec![0xc1],
            vec![0x01],
            vec![0xa1, b'a'],
            vec![0xa1, 0xff],
            vec![0xc4, 0x02, 0x00, 0xff],
            vec![0xd4, 0x01, 0xaa],
            vec![0xc7, 0x02, 0xff, 0x01, 0x02],
            vec![0x92, 0x01, 0x02],
            vec![0x81, 0xa1, b'a', 0x01],
            vec![0x92, 0x01],
            vec![0x81, 0xa1, b'a'],
            vec![0xd9, 0x02, b'a'],
            vec![0x01, 0x02],
        ];

        let mut large_binary_array = Vec::with_capacity(1024 * 1024 + 6);
        large_binary_array.extend_from_slice(&[0x91, 0xc6]);
        large_binary_array.extend_from_slice(&(1024_u32 * 1024).to_be_bytes());
        large_binary_array.resize(1024 * 1024 + 6, 0x5a);
        cases.push(large_binary_array);

        for data in cases {
            assert_eq!(validate_msgpack_value(&data), owning_validate_value(&data));
            assert_eq!(validate_msgpack_array(&data), owning_validate_array(&data));
            assert_eq!(validate_msgpack_map(&data), owning_validate_map(&data));
        }

        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                for depth in [1, 1023, 1024, 1025] {
                    let mut nested = vec![0x91; depth];
                    nested.push(0xc0);
                    assert_eq!(
                        validate_msgpack_value(&nested),
                        owning_validate_value(&nested)
                    );
                    assert_eq!(
                        validate_msgpack_array(&nested),
                        owning_validate_array(&nested)
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
