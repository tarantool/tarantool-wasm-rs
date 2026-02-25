use crate::bindings::tarantool::tarantool::types::{BoxError, FieldType};
use core::str::FromStr;
use tarantool::error::TarantoolErrorCode;

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::Boolean => "boolean",
            FieldType::Integer => "integer",
            FieldType::Unsigned => "unsigned",
            FieldType::Double => "double",
            FieldType::Number => "number",
            FieldType::Decimal => "decimal",
            FieldType::String => "string",
            FieldType::Varbinary => "varbinary",
            FieldType::Uuid => "uuid",
            FieldType::Datetime => "datetime",
            FieldType::Array => "array",
            FieldType::Map => "map",
            FieldType::Scalar => "scalar",
        }
    }
}

impl FromStr for FieldType {
    type Err = BoxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "boolean" => Ok(FieldType::Boolean),
            "integer" => Ok(FieldType::Integer),
            "unsigned" => Ok(FieldType::Unsigned),
            "double" => Ok(FieldType::Double),
            "number" => Ok(FieldType::Number),
            "decimal" => Ok(FieldType::Decimal),
            "string" => Ok(FieldType::String),
            "varbinary" => Ok(FieldType::Varbinary),
            "uuid" => Ok(FieldType::Uuid),
            "datetime" => Ok(FieldType::Datetime),
            "array" => Ok(FieldType::Array),
            "map" => Ok(FieldType::Map),
            "scalar" => Ok(FieldType::Scalar),
            _ => Err(BoxError::new(
                u32::from(TarantoolErrorCode::IllegalParams),
                format!("Unsupported field type: {s}"),
            )),
        }
    }
}
