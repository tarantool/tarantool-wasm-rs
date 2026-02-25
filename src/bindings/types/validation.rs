use rmpv::decode::read_value;

pub fn validate_msgpack_array(data: &[u8]) -> Result<(), String> {
    if data.is_empty() {
        return Err("Tuple data is empty".to_string());
    }
    match read_value(&mut &data[..]) {
        Ok(val) if val.is_array() => Ok(()),
        Ok(other) => Err(format!("Expected MsgPack array, got {other:?}")),
        Err(e) => Err(format!("Invalid MsgPack data: {e}")),
    }
}

pub fn validate_msgpack_map(data: &[u8]) -> Result<(), String> {
    if data.is_empty() {
        return Err("Map data is empty".to_string());
    }
    match read_value(&mut &data[..]) {
        Ok(val) if val.is_map() => Ok(()),
        Ok(other) => Err(format!("Expected MsgPack map, got {other:?}")),
        Err(e) => Err(format!("Invalid MsgPack data: {e}")),
    }
}
