//! Serde MessagePack with lossless Tarantool extension types.
//! Use `Value::Ext` for unknown extensions and `Value::Map` for ordered/duplicate keys.
use crate::{Error, Result};
use rmpv::Value;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Cursor;

pub fn pack(value: &impl Serialize) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(value)?)
}

pub fn unpack<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut de = rmp_serde::Deserializer::new(Cursor::new(bytes));
    let value = T::deserialize(&mut de)?;
    if de.get_ref().position() != bytes.len() as u64 {
        return Err(Error::Invalid("trailing MessagePack bytes"));
    }
    Ok(value)
}

pub(crate) fn pack_array(value: &impl Serialize) -> Result<Vec<u8>> {
    let bytes = pack(value)?;
    if !matches!(bytes.first(), Some(0x90..=0x9f | 0xdc | 0xdd)) {
        return Err(Error::Invalid("expected a MessagePack array"));
    }
    Ok(bytes)
}

fn invalid() -> Error {
    Error::Invalid("invalid Tarantool extension payload")
}

fn integer(v: &Value) -> Result<i64> {
    v.as_i64().ok_or_else(invalid)
}

fn extension(v: Value, tag: i8) -> Result<Vec<u8>> {
    match v {
        Value::Ext(t, bytes) if t == tag => Ok(bytes),
        _ => Err(invalid()),
    }
}

macro_rules! serde_extension {
    ($ty:ty) => {
        impl Serialize for $ty {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                self.to_value()
                    .map_err(serde::ser::Error::custom)?
                    .serialize(s)
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(
                d: D,
            ) -> std::result::Result<Self, D::Error> {
                Self::try_from(Value::deserialize(d)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

/// UUID in network byte order, without platform-dependent field swapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uuid(pub [u8; 16]);

impl Uuid {
    pub fn to_value(&self) -> Result<Value> {
        Ok(Value::Ext(2, self.0.to_vec()))
    }
}

impl TryFrom<Value> for Uuid {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self> {
        Ok(Self(extension(v, 2)?.try_into().map_err(|_| invalid())?))
    }
}

serde_extension!(Uuid);

/// Exact signed coefficient * 10^-scale, up to 38 digits. No float conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    pub digits: String,
    pub scale: i32,
    pub negative: bool,
}

impl Decimal {
    pub fn to_value(&self) -> Result<Value> {
        if self.digits.is_empty()
            || self.digits.len() > 38
            || !self.digits.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }
        let mut payload = pack(&self.scale)?;
        let mut high = self.digits.len().is_multiple_of(2).then_some(0);
        for digit in self.digits.bytes().map(|b| b - b'0') {
            if let Some(first) = high.take() {
                payload.push(first << 4 | digit);
            } else {
                high = Some(digit);
            }
        }
        payload.push(high.unwrap() << 4 | if self.negative { 0xd } else { 0xc });
        Ok(Value::Ext(1, payload))
    }
}

impl TryFrom<Value> for Decimal {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self> {
        let payload = extension(v, 1)?;
        let mut input = Cursor::new(payload.as_slice());
        let scale = integer(&rmpv::decode::read_value(&mut input).map_err(|_| invalid())?)?
            .try_into()
            .map_err(|_| invalid())?;
        let bcd = &payload[input.position() as usize..];
        if bcd.is_empty() || bcd.len() > 20 {
            return Err(invalid());
        }
        let sign = bcd[bcd.len() - 1] & 15;
        if sign < 10 {
            return Err(invalid());
        }
        let mut digits = String::new();
        for (i, byte) in bcd.iter().enumerate() {
            for nibble in [byte >> 4, byte & 15]
                .into_iter()
                .take(if i + 1 == bcd.len() { 1 } else { 2 })
            {
                if nibble > 9 {
                    return Err(invalid());
                }
                digits.push(char::from(b'0' + nibble));
            }
        }
        let digits = digits.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        if digits.len() > 38 {
            return Err(invalid());
        }
        Ok(Self {
            digits: digits.into(),
            scale,
            negative: matches!(sign, 0xb | 0xd),
        })
    }
}

serde_extension!(Decimal);

/// Seconds since epoch, nanoseconds and original timezone metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Datetime {
    pub seconds: i64,
    pub nanoseconds: u32,
    pub tz_offset: i16,
    pub tz_index: i16,
}

impl Datetime {
    pub fn to_value(&self) -> Result<Value> {
        if self.nanoseconds >= 1_000_000_000 {
            return Err(invalid());
        }
        let mut payload = self.seconds.to_le_bytes().to_vec();
        if self.nanoseconds != 0 || self.tz_offset != 0 || self.tz_index != 0 {
            payload.extend(self.nanoseconds.to_le_bytes());
            payload.extend(self.tz_offset.to_le_bytes());
            payload.extend(self.tz_index.to_le_bytes());
        }
        Ok(Value::Ext(4, payload))
    }
}

impl TryFrom<Value> for Datetime {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self> {
        let p = extension(v, 4)?;
        if p.len() != 8 && p.len() != 16 {
            return Err(invalid());
        }
        let mut result = Self {
            seconds: i64::from_le_bytes(p[..8].try_into().unwrap()),
            ..Self::default()
        };
        if p.len() == 16 {
            result.nanoseconds = u32::from_le_bytes(p[8..12].try_into().unwrap());
            result.tz_offset = i16::from_le_bytes(p[12..14].try_into().unwrap());
            result.tz_index = i16::from_le_bytes(p[14..16].try_into().unwrap());
        }
        if result.nanoseconds >= 1_000_000_000 {
            return Err(invalid());
        }
        Ok(result)
    }
}

serde_extension!(Datetime);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i64)]
pub enum Adjust {
    #[default]
    Excess = 0,
    None = 1,
    Last = 2,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interval {
    pub years: i64,
    pub months: i64,
    pub weeks: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub nanoseconds: i64,
    pub adjust: Adjust,
}

impl Interval {
    pub fn to_value(&self) -> Result<Value> {
        let fields = [
            self.years,
            self.months,
            self.weeks,
            self.days,
            self.hours,
            self.minutes,
            self.seconds,
            self.nanoseconds,
            self.adjust as i64,
        ];
        let mut payload = vec![fields.iter().filter(|n| **n != 0).count() as u8];
        for (i, n) in fields.iter().enumerate() {
            if *n != 0 {
                payload.push(i as u8);
                payload.extend(pack(n)?);
            }
        }
        Ok(Value::Ext(6, payload))
    }
}

impl TryFrom<Value> for Interval {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self> {
        let p = extension(v, 6)?;
        let count = *p.first().ok_or_else(invalid)?;
        if count > 9 {
            return Err(invalid());
        }
        let mut input = Cursor::new(p.as_slice());
        input.set_position(1);
        let mut fields = [0; 9];
        let mut seen = [false; 9];
        for _ in 0..count {
            let i = *p.get(input.position() as usize).ok_or_else(invalid)? as usize;
            input.set_position(input.position() + 1);
            if i >= 9 || seen[i] {
                return Err(invalid());
            }
            seen[i] = true;
            fields[i] = integer(&rmpv::decode::read_value(&mut input).map_err(|_| invalid())?)?;
        }
        if input.position() != p.len() as u64 {
            return Err(invalid());
        }
        let adjust = match fields[8] {
            0 => Adjust::Excess,
            1 => Adjust::None,
            2 => Adjust::Last,
            _ => return Err(invalid()),
        };
        Ok(Self {
            years: fields[0],
            months: fields[1],
            weeks: fields[2],
            days: fields[3],
            hours: fields[4],
            minutes: fields[5],
            seconds: fields[6],
            nanoseconds: fields[7],
            adjust,
        })
    }
}

serde_extension!(Interval);

/// Error extension numeric attributes, including the stack at key 0.
/// Retains future attributes and frame fields without interpretation or loss.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorValue(pub Vec<(Value, Value)>);

impl ErrorValue {
    pub fn to_value(&self) -> Result<Value> {
        validate_error(&self.0)?;
        Ok(Value::Ext(3, pack(&Value::Map(self.0.clone()))?))
    }
}

fn validate_error(fields: &[(Value, Value)]) -> Result<()> {
    let mut keys = std::collections::HashSet::new();
    let mut stack = false;
    for (key, value) in fields {
        let key = key.as_u64().ok_or_else(invalid)?;
        if !keys.insert(key) {
            return Err(invalid());
        }
        if key == 0 {
            let frames = value.as_array().ok_or_else(invalid)?;
            if frames.is_empty() || frames.iter().any(|f| !matches!(f, Value::Map(_))) {
                return Err(invalid());
            }
            stack = true;
        }
    }
    if !stack {
        return Err(invalid());
    }
    Ok(())
}

impl TryFrom<Value> for ErrorValue {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self> {
        let payload = extension(v, 3)?;
        match unpack(&payload)? {
            Value::Map(fields) => {
                validate_error(&fields)?;
                Ok(Self(fields))
            }
            _ => Err(invalid()),
        }
    }
}

serde_extension!(ErrorValue);

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + DeserializeOwned + std::fmt::Debug + PartialEq>(value: T) {
        assert_eq!(unpack::<T>(&pack(&value).unwrap()).unwrap(), value);
    }

    #[test]
    fn serde_values_and_strict_input() {
        roundtrip((i64::MIN, u64::MAX, "Привет\0🙂".to_string(), Some(42)));
        roundtrip(Value::Map(vec![
            (Value::Nil, Value::Binary(vec![0, 255])),
            (Value::Nil, Value::Ext(-42, vec![1, 2])),
        ]));
        for bytes in [&[][..], &[0xc1], &[0xc0, 0], &[0x92, 1]] {
            assert!(unpack::<Value>(bytes).is_err());
        }
        assert!(unpack::<String>(&[0xd9, 2, 0xff, 0xff]).is_err());
        assert!(pack_array(&1).is_err());
        assert_eq!(
            pack_array(&(1, "name")).unwrap(),
            vec![0x92, 1, 0xa4, b'n', b'a', b'm', b'e']
        );
    }

    #[test]
    fn extension_wire_vectors() {
        let uuid = Uuid([0xff; 16]);
        assert_eq!(
            pack(&uuid).unwrap(),
            [vec![0xd8, 2], vec![0xff; 16]].concat()
        );
        roundtrip(uuid);
        let decimal = Decimal {
            digits: "12345".into(),
            scale: 2,
            negative: true,
        };
        assert_eq!(pack(&decimal).unwrap(), vec![0xd6, 1, 2, 0x12, 0x34, 0x5d]);
        roundtrip(decimal);
        roundtrip(Decimal {
            digits: "12345678901234567890123456789012345678".into(),
            scale: -10,
            negative: false,
        });
        roundtrip(Datetime {
            seconds: i64::MIN,
            nanoseconds: 999999999,
            tz_offset: -180,
            tz_index: 42,
        });
        assert_eq!(
            pack(&Datetime::default()).unwrap(),
            [vec![0xd7, 4], vec![0; 8]].concat()
        );
        roundtrip(Interval {
            years: -10,
            seconds: i64::MAX,
            nanoseconds: -999999999,
            adjust: Adjust::Last,
            ..Interval::default()
        });
        assert_eq!(pack(&Interval::default()).unwrap(), vec![0xd4, 6, 0]);
        roundtrip(ErrorValue(vec![
            (
                0.into(),
                Value::Array(vec![Value::Map(vec![
                    (0.into(), "ClientError".into()),
                    (3.into(), "error".into()),
                    (7.into(), Value::Ext(99, vec![])),
                ])]),
            ),
            (42.into(), "future".into()),
        ]));
    }

    #[test]
    fn malformed_extensions() {
        for v in [
            Value::Ext(1, vec![]),
            Value::Ext(1, vec![0, 0x1a, 0x0c]),
            Value::Ext(1, vec![0, 0x12]),
            Value::Ext(2, vec![0; 15]),
            Value::Ext(4, vec![0; 9]),
            Value::Ext(6, vec![2, 0, 1, 0, 2]),
            Value::Ext(6, vec![1, 8, 3]),
            Value::Ext(6, vec![0, 0]),
        ] {
            let data = pack(&v).unwrap();
            let valid = match v {
                Value::Ext(1, _) => unpack::<Decimal>(&data).is_ok(),
                Value::Ext(2, _) => unpack::<Uuid>(&data).is_ok(),
                Value::Ext(4, _) => unpack::<Datetime>(&data).is_ok(),
                _ => unpack::<Interval>(&data).is_ok(),
            };
            assert!(!valid);
        }
        assert!(pack(&Decimal {
            digits: "9".repeat(39),
            scale: 0,
            negative: false
        })
        .is_err());
        assert!(pack(&Datetime {
            nanoseconds: 1_000_000_000,
            ..Datetime::default()
        })
        .is_err());
        assert!(pack(&ErrorValue(vec![])).is_err());
        assert!(unpack::<Uuid>(&pack(&Value::Ext(99, vec![0; 16])).unwrap()).is_err());
    }
}
