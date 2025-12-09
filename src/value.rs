use std::fmt::Display;

use serde::ser::Serialize;

use crate::{string::SapString, structure::SapStructure, table::SapTable};

/// Represents a value in the SAP RFC protocol, which can be of various types.
/// This enum can hold an empty value, a string, an integer, a table, or a structure.
/// It implements serialization for use with Serde, allowing it to be easily
/// converted to formats like JSON.
#[derive(Debug)]
pub enum Value {
    Empty,
    String(SapString),
    Int(i64),
    Table(SapTable),
    Structure(SapStructure),
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Empty => serializer.serialize_unit(),
            Value::String(s) => serializer.serialize_str(&String::from(s)),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Table(t) => t.serialize(serializer),
            Value::Structure(s) => s.serialize(serializer),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(s) => write!(f, "{}", String::from(s)),
            Value::Int(i) => write!(f, "{i}"),
            Value::Table(_) => todo!(),
            Value::Structure(_) => todo!(),
            Value::Empty => todo!(),
        }
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(SapString::from(value))
    }
}
