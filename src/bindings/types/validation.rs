use rmpv::decode::read_value_ref;
use std::io::Cursor;

const FAST_SCAN_MAX_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueKind {
    Array,
    Map,
    Other,
}

struct Scanner<'a> {
    data: &'a [u8],
    position: usize,
}

impl Scanner<'_> {
    fn read_u8(&mut self) -> Option<u8> {
        let value = *self.data.get(self.position)?;
        self.position += 1;
        Some(value)
    }

    fn read_u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.take(2)?.try_into().ok()?;
        Some(u16::from_be_bytes(bytes))
    }

    fn read_u32(&mut self) -> Option<u32> {
        let bytes: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(u32::from_be_bytes(bytes))
    }

    fn take(&mut self, len: usize) -> Option<&[u8]> {
        let end = self.position.checked_add(len)?;
        let bytes = self.data.get(self.position..end)?;
        self.position = end;
        Some(bytes)
    }

    fn skip(&mut self, len: u64) -> Option<()> {
        self.take(usize::try_from(len).ok()?).map(|_| ())
    }

    fn read_marker(&mut self) -> Option<(ValueKind, u64)> {
        let marker = self.read_u8()?;
        let result = match marker {
            0x00..=0x7f | 0xc0 | 0xc2..=0xc3 | 0xe0..=0xff => (ValueKind::Other, 0),
            0x80..=0x8f => (ValueKind::Map, u64::from(marker & 0x0f) * 2),
            0x90..=0x9f => (ValueKind::Array, u64::from(marker & 0x0f)),
            0xa0..=0xbf => {
                self.skip(u64::from(marker & 0x1f))?;
                (ValueKind::Other, 0)
            }
            0xc4 | 0xd9 => {
                let len = u64::from(self.read_u8()?);
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xc5 | 0xda => {
                let len = u64::from(self.read_u16()?);
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xc6 | 0xdb => {
                let len = u64::from(self.read_u32()?);
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xc7 => {
                let len = u64::from(self.read_u8()?).checked_add(1)?;
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xc8 => {
                let len = u64::from(self.read_u16()?).checked_add(1)?;
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xc9 => {
                let len = u64::from(self.read_u32()?).checked_add(1)?;
                self.skip(len)?;
                (ValueKind::Other, 0)
            }
            0xca | 0xce | 0xd2 => {
                self.skip(4)?;
                (ValueKind::Other, 0)
            }
            0xcb | 0xcf | 0xd3 => {
                self.skip(8)?;
                (ValueKind::Other, 0)
            }
            0xcc | 0xd0 => {
                self.skip(1)?;
                (ValueKind::Other, 0)
            }
            0xcd | 0xd1 => {
                self.skip(2)?;
                (ValueKind::Other, 0)
            }
            0xd4 => {
                self.skip(2)?;
                (ValueKind::Other, 0)
            }
            0xd5 => {
                self.skip(3)?;
                (ValueKind::Other, 0)
            }
            0xd6 => {
                self.skip(5)?;
                (ValueKind::Other, 0)
            }
            0xd7 => {
                self.skip(9)?;
                (ValueKind::Other, 0)
            }
            0xd8 => {
                self.skip(17)?;
                (ValueKind::Other, 0)
            }
            0xdc => (ValueKind::Array, u64::from(self.read_u16()?)),
            0xdd => (ValueKind::Array, u64::from(self.read_u32()?)),
            0xde => (ValueKind::Map, u64::from(self.read_u16()?) * 2),
            0xdf => (ValueKind::Map, u64::from(self.read_u32()?) * 2),
            0xc1 => return None,
        };
        Some(result)
    }
}

fn scan_value(data: &[u8]) -> Option<(ValueKind, usize)> {
    let mut scanner = Scanner { data, position: 0 };
    let (root_kind, root_children) = scanner.read_marker()?;
    if root_children == 0 {
        return Some((root_kind, scanner.position));
    }

    let mut remaining = [0_u64; FAST_SCAN_MAX_DEPTH];
    remaining[0] = root_children;
    let mut depth = 1;

    loop {
        let (_, children) = scanner.read_marker()?;
        if children != 0 {
            if depth == FAST_SCAN_MAX_DEPTH {
                return None;
            }
            remaining[depth] = children;
            depth += 1;
            continue;
        }

        loop {
            remaining[depth - 1] -= 1;
            if remaining[depth - 1] != 0 {
                break;
            }
            depth -= 1;
            if depth == 0 {
                return Some((root_kind, scanner.position));
            }
        }
    }
}

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

fn scan_complete_value(data: &[u8], empty_message: &str) -> Result<Option<ValueKind>, String> {
    if data.is_empty() {
        return Err(empty_message.to_string());
    }
    let Some((kind, position)) = scan_value(data) else {
        return Ok(None);
    };
    if position != data.len() {
        return Err("Trailing bytes after MsgPack value".to_string());
    }
    Ok(Some(kind))
}

pub fn validate_msgpack_value(data: &[u8]) -> Result<(), String> {
    match scan_complete_value(data, "MessagePack data is empty")? {
        Some(_) => Ok(()),
        None => read_complete_value(data, "MessagePack data is empty").map(|_| ()),
    }
}

pub fn validate_msgpack_array(data: &[u8]) -> Result<(), String> {
    if scan_complete_value(data, "Tuple data is empty")? == Some(ValueKind::Array) {
        return Ok(());
    }
    match read_complete_value(data, "Tuple data is empty")? {
        rmpv::ValueRef::Array(_) => Ok(()),
        other => Err(format!(
            "Expected MsgPack array, got {:?}",
            other.to_owned()
        )),
    }
}

pub fn validate_msgpack_map(data: &[u8]) -> Result<(), String> {
    if scan_complete_value(data, "Map data is empty")? == Some(ValueKind::Map) {
        return Ok(());
    }
    match read_complete_value(data, "Map data is empty")? {
        rmpv::ValueRef::Map(_) => Ok(()),
        other => Err(format!("Expected MsgPack map, got {:?}", other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::{scan_value, validate_msgpack_array, validate_msgpack_map, validate_msgpack_value};
    use std::io::Cursor;

    fn minimal_value_for_marker(marker: u8) -> Option<Vec<u8>> {
        let mut data = vec![marker];
        match marker {
            0x00..=0x7f | 0xc0 | 0xc2..=0xc3 | 0xe0..=0xff => {}
            0x80..=0x8f => data.resize(1 + usize::from(marker & 0x0f) * 2, 0xc0),
            0x90..=0x9f => data.resize(1 + usize::from(marker & 0x0f), 0xc0),
            0xa0..=0xbf => data.resize(1 + usize::from(marker & 0x1f), 0),
            0xc4 | 0xd9 => data.push(0),
            0xc5 | 0xda | 0xdc | 0xde => data.extend_from_slice(&[0, 0]),
            0xc6 | 0xdb | 0xdd | 0xdf => data.extend_from_slice(&[0, 0, 0, 0]),
            0xc7 => data.extend_from_slice(&[0, 0]),
            0xc8 => data.extend_from_slice(&[0, 0, 0]),
            0xc9 => data.extend_from_slice(&[0, 0, 0, 0, 0]),
            0xca | 0xce | 0xd2 => data.resize(5, 0),
            0xcb | 0xcf | 0xd3 => data.resize(9, 0),
            0xcc | 0xd0 => data.push(0),
            0xcd | 0xd1 => data.extend_from_slice(&[0, 0]),
            0xd4 => data.resize(3, 0),
            0xd5 => data.resize(4, 0),
            0xd6 => data.resize(6, 0),
            0xd7 => data.resize(10, 0),
            0xd8 => data.resize(18, 0),
            0xc1 => return None,
        }
        Some(data)
    }

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
    fn fast_scanner_handles_every_valid_marker() {
        for marker in u8::MIN..=u8::MAX {
            let Some(data) = minimal_value_for_marker(marker) else {
                continue;
            };
            let (_, position) = scan_value(&data)
                .unwrap_or_else(|| panic!("fast scanner rejected marker 0x{marker:02x}"));
            assert_eq!(position, data.len(), "marker 0x{marker:02x}");

            let mut remaining = data.as_slice();
            rmpv::decode::read_value_ref(&mut remaining).unwrap();
            assert!(remaining.is_empty(), "marker 0x{marker:02x}");
        }
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

        let mut wide_array = Vec::with_capacity(100_005);
        wide_array.extend_from_slice(&[0xdd, 0x00, 0x01, 0x86, 0xa0]);
        wide_array.resize(100_005, 0x01);
        cases.push(wide_array);

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
