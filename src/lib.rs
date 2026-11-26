use log::trace;
use std::sync::{Mutex, MutexGuard};
use string::SapString;

pub use connection::Connection;
pub use error::{AbapMessage, RfcError, RfcResult, SdkError};
pub use function::Function;
pub use value::Value;

use crate::librfc::{
    RfcAppendNewRow, RfcCreateStructure, RfcCreateTable, RfcGetFieldCount, RfcGetFieldDescByIndex,
    RfcSetChars, RfcSetStructure, RfcSetTable, RfcSetXString, RFC_DATA_CONTAINER, RFC_ERROR_INFO,
    RFC_FIELD_DESC, RFC_PARAMETER_DESC, RFC_RC, RFC_TYPE_DESC_HANDLE, _RFCTYPE_RFCTYPE_CHAR,
    _RFCTYPE_RFCTYPE_STRING, _RFCTYPE_RFCTYPE_STRUCTURE, _RFCTYPE_RFCTYPE_TABLE, _RFC_FIELD_DESC,
    _RFC_RC_RFC_OK, _RFC_TYPE_DESC_HANDLE,
};

static CONNECT_COUNT: Mutex<i32> = Mutex::new(0);
static INI_PATH_INITIALIZED: Mutex<bool> = Mutex::new(false);

/// Sperrt einen Mutex; eine vergiftete Sperre wird als `RfcError::Internal` gemeldet.
fn lock<T>(m: &Mutex<T>) -> RfcResult<MutexGuard<'_, T>> {
    m.lock()
        .map_err(|e| RfcError::Internal(format!("Mutex vergiftet: {e}")))
}

#[allow(
    dead_code,
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    clippy::all
)]
mod librfc {
    /// ABAP-Datentyp eines Feldes (RFCTYPE). Unbekannte Typen werden als `Unknown` abgebildet
    /// statt per `transmute` in ein Enum umgewandelt (das wäre undefiniertes Verhalten).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RfcType {
        Char,
        Date,
        Time,
        Byte,
        Float,
        Int1,
        Int2,
        Int8,
        Bcd,
        Num,
        Int,
        String,
        Structure,
        Table,
        XString,
        Unknown(i32),
    }

    impl From<i32> for RfcType {
        fn from(value: i32) -> Self {
            match value {
                _RFCTYPE_RFCTYPE_CHAR => RfcType::Char,
                _RFCTYPE_RFCTYPE_DATE => RfcType::Date,
                _RFCTYPE_RFCTYPE_TIME => RfcType::Time,
                _RFCTYPE_RFCTYPE_BYTE => RfcType::Byte,
                _RFCTYPE_RFCTYPE_FLOAT => RfcType::Float,
                _RFCTYPE_RFCTYPE_INT1 => RfcType::Int1,
                _RFCTYPE_RFCTYPE_INT2 => RfcType::Int2,
                _RFCTYPE_RFCTYPE_INT8 => RfcType::Int8,
                _RFCTYPE_RFCTYPE_BCD => RfcType::Bcd,
                _RFCTYPE_RFCTYPE_NUM => RfcType::Num,
                _RFCTYPE_RFCTYPE_INT => RfcType::Int,
                _RFCTYPE_RFCTYPE_STRING => RfcType::String,
                _RFCTYPE_RFCTYPE_STRUCTURE => RfcType::Structure,
                _RFCTYPE_RFCTYPE_TABLE => RfcType::Table,
                _RFCTYPE_RFCTYPE_XSTRING => RfcType::XString,
                other => RfcType::Unknown(other),
            }
        }
    }

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub mod error;
pub mod rfc_param;
mod string;

pub mod connection;

mod function;

mod structure;
mod table;

pub mod value;

/// Creates a new `RFC_FIELD_DESC` with default values.
fn field_descriptor() -> _RFC_FIELD_DESC {
    RFC_FIELD_DESC {
        name: zero(),
        type_: 0,
        nucLength: 0,
        nucOffset: 0,
        ucLength: 0,
        ucOffset: 0,
        decimals: 0,
        typeDescHandle: std::ptr::null_mut(),
        extendedDescription: std::ptr::null_mut(),
    }
}

/// Creates a zero-initialized `RFC_PARAMETER_DESC` structure.
fn parameter_description() -> RFC_PARAMETER_DESC {
    RFC_PARAMETER_DESC {
        name: zero(),
        type_: 0,
        direction: 0,
        nucLength: 0,
        ucLength: 0,
        decimals: 0,
        typeDescHandle: std::ptr::null_mut(),
        defaultValue: zero(),
        parameterText: zero(),
        optional: 0,
        extendedDescription: std::ptr::null_mut(),
    }
}

/// Creates a zero-initialized array of `u16` with the specified size.
fn zero<const N: usize>() -> [u16; N] {
    [0; N]
}

/// Creates a new, zero-initialized `RFC_ERROR_INFO`.
fn error_info() -> RFC_ERROR_INFO {
    RFC_ERROR_INFO {
        abapMsgClass: [0; 21],
        abapMsgNumber: [0; 4],
        abapMsgType: [0; 2],
        abapMsgV1: [0; 51],
        abapMsgV2: [0; 51],
        abapMsgV3: [0; 51],
        abapMsgV4: [0; 51],
        code: 0,
        group: 0,
        key: [0; 128],
        message: [0; 512],
    }
}

fn sap_text(buf: &[u16]) -> String {
    String::from(&SapString::from(buf))
}

/// Übersetzt `RFC_ERROR_INFO` in einen `RfcError::Sdk`.
fn sdk_error(ei: &RFC_ERROR_INFO) -> RfcError {
    RfcError::Sdk(Box::new(sdk_error_info(ei)))
}

fn sdk_error_info(ei: &RFC_ERROR_INFO) -> SdkError {
    let class = sap_text(&ei.abapMsgClass);
    let msg_type = sap_text(&ei.abapMsgType);
    let number = sap_text(&ei.abapMsgNumber);
    let abap = if class.is_empty() && msg_type.is_empty() && number.is_empty() {
        None
    } else {
        Some(AbapMessage {
            class,
            msg_type,
            number,
            v1: sap_text(&ei.abapMsgV1),
            v2: sap_text(&ei.abapMsgV2),
            v3: sap_text(&ei.abapMsgV3),
            v4: sap_text(&ei.abapMsgV4),
        })
    };
    SdkError {
        code: ei.code,
        group: ei.group,
        key: sap_text(&ei.key),
        message: sap_text(&ei.message),
        abap,
    }
}

/// Wertet Rückgabecode und Fehlerinfo einer RFC-Funktion aus.
pub(crate) fn check_rc(rc: RFC_RC, ei: &RFC_ERROR_INFO) -> RfcResult<()> {
    if rc == _RFC_RC_RFC_OK && ei.code == _RFC_RC_RFC_OK {
        Ok(())
    } else {
        Err(sdk_error(ei))
    }
}

/// Wie `check_rc`, für Funktionen, die einen Handle liefern: ein Null-Pointer ist ein Fehler.
pub(crate) fn check_ptr<T>(
    ptr: *mut T,
    ei: &RFC_ERROR_INFO,
    operation: &'static str,
) -> RfcResult<*mut T> {
    if ei.code != _RFC_RC_RFC_OK {
        Err(sdk_error(ei))
    } else if ptr.is_null() {
        Err(RfcError::NullHandle { operation })
    } else {
        Ok(ptr)
    }
}

/// Liefert den JSON-Typ eines Werts, ohne den Wert selbst (der Nutzdaten enthalten kann) auszugeben.
pub(crate) fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

pub(crate) fn set_chars(
    cont: *mut RFC_DATA_CONTAINER,
    name: &str,
    value: SapString,
) -> RfcResult<()> {
    let mut ei = error_info();
    let str_name = SapString::from(name);
    let rc = unsafe {
        RfcSetChars(
            cont,
            str_name.raw_pointer(),
            value.raw_pointer(),
            value.len() as u32,
            &mut ei,
        )
    };
    trace!("set value for {}: {:?} -> {}", name, value, rc);
    check_rc(rc, &ei)
}

/// Setzt ein XSTRING-Feld aus den UTF-8-Bytes eines Strings.
pub(crate) fn set_xstring_from_str(
    cont: *mut RFC_DATA_CONTAINER,
    name: &str,
    value: &str,
) -> RfcResult<()> {
    let mut ei = error_info();
    let str_name = SapString::from(name);
    let bytes = value.as_bytes();
    let rc = unsafe {
        RfcSetXString(
            cont,
            str_name.raw_pointer(),
            bytes.as_ptr(),
            bytes.len() as u32,
            &mut ei,
        )
    };
    trace!("set value for {}: {:?} -> {}", name, bytes, rc);
    check_rc(rc, &ei)
}

/// Setzt ein CHAR-/STRING-Feld aus einem `&str`.
fn set_chars_from_str(cont: *mut RFC_DATA_CONTAINER, name: &str, value: &str) -> RfcResult<()> {
    set_chars(cont, name, SapString::from(value))
}

/// Setzt eine Struktur im Container aus einer JSON-Map.
fn set_structure_from_type_handle(
    cont: *mut RFC_DATA_CONTAINER,
    name: &str,
    type_handle: RFC_TYPE_DESC_HANDLE,
    value: &serde_json::Map<String, serde_json::Value>,
) -> RfcResult<()> {
    let mut ei = error_info();
    let structure_handle = unsafe { RfcCreateStructure(type_handle, &mut ei) };
    let structure_handle = check_ptr(structure_handle, &ei, "RfcCreateStructure")?;
    fill_structure(structure_handle, type_handle, value)?;

    let name_sap = SapString::from(name);
    let mut ei = error_info();
    let rc = unsafe { RfcSetStructure(cont, name_sap.raw_pointer(), structure_handle, &mut ei) };
    check_rc(rc, &ei)
}

/// Füllt einen Structure-Container aus einer JSON-Map.
/// Jeder JSON-Schlüssel muss einem Feld der Struktur entsprechen und einen passenden Typ haben;
/// Felder, die nicht in der JSON-Map vorkommen, bleiben unverändert.
fn fill_structure(
    row_handle: *mut RFC_DATA_CONTAINER,
    row_type_handle: *mut _RFC_TYPE_DESC_HANDLE,
    values_map: &serde_json::Map<String, serde_json::Value>,
) -> RfcResult<()> {
    let mut ei = error_info();
    let mut count = 0;
    let rc = unsafe { RfcGetFieldCount(row_type_handle, &mut count, &mut ei) };
    check_rc(rc, &ei)?;

    let mut field_names: Vec<String> = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let mut field_descr = field_descriptor();
        let mut ei = error_info();
        let rc = unsafe { RfcGetFieldDescByIndex(row_type_handle, idx, &mut field_descr, &mut ei) };
        check_rc(rc, &ei)?;

        let name = String::from(&SapString::new(field_descr.name));
        field_names.push(name.clone());
        let Some(v) = values_map.get(&name) else {
            continue;
        };
        match field_descr.type_ {
            _RFCTYPE_RFCTYPE_CHAR | _RFCTYPE_RFCTYPE_STRING => match v {
                serde_json::Value::String(s) => set_chars_from_str(row_handle, &name, s)?,
                other => {
                    return Err(RfcError::TypeMismatch {
                        name,
                        expected: "string",
                        found: json_kind(other),
                    })
                }
            },
            _RFCTYPE_RFCTYPE_STRUCTURE => match v {
                serde_json::Value::Object(obj) => set_structure_from_type_handle(
                    row_handle,
                    &name,
                    field_descr.typeDescHandle,
                    obj,
                )?,
                other => {
                    return Err(RfcError::TypeMismatch {
                        name,
                        expected: "object",
                        found: json_kind(other),
                    })
                }
            },
            _RFCTYPE_RFCTYPE_TABLE => match v {
                serde_json::Value::Array(arr) => set_table_from_type_handle(
                    row_handle,
                    &name,
                    field_descr.typeDescHandle,
                    arr,
                )?,
                other => {
                    return Err(RfcError::TypeMismatch {
                        name,
                        expected: "array",
                        found: json_kind(other),
                    })
                }
            },
            other => {
                return Err(RfcError::UnsupportedType {
                    name,
                    type_code: other,
                })
            }
        }
    }

    if let Some(unknown) = values_map.keys().find(|k| !field_names.contains(k)) {
        return Err(RfcError::UnknownField {
            name: unknown.clone(),
        });
    }
    Ok(())
}

/// Setzt eine Tabelle im Container aus einem JSON-Array von Objekten.
fn set_table_from_type_handle(
    cont: *mut RFC_DATA_CONTAINER,
    name: &str,
    type_handle: RFC_TYPE_DESC_HANDLE,
    value: &[serde_json::Value],
) -> RfcResult<()> {
    let mut ei = error_info();
    let table_handle = unsafe { RfcCreateTable(type_handle, &mut ei) };
    let table_handle = check_ptr(table_handle, &ei, "RfcCreateTable")?;

    for v in value {
        match v {
            serde_json::Value::Object(obj) => {
                let mut ei = error_info();
                let row_handle = unsafe { RfcAppendNewRow(table_handle, &mut ei) };
                let row_handle = check_ptr(row_handle, &ei, "RfcAppendNewRow")?;
                fill_structure(row_handle, type_handle, obj)?;
            }
            other => {
                return Err(RfcError::TypeMismatch {
                    name: name.to_string(),
                    expected: "object (table row)",
                    found: json_kind(other),
                })
            }
        }
    }

    trace!("Setting table: {}", name);
    let name_sap = SapString::from(name);
    let mut ei = error_info();
    let rc = unsafe { RfcSetTable(cont, name_sap.raw_pointer(), table_handle, &mut ei) };
    check_rc(rc, &ei)
}

#[cfg(test)]
mod tests {
    use crate::librfc::RfcType;

    #[test]
    fn unknown_sap_type_is_not_ub() {
        assert_eq!(RfcType::from(0), RfcType::Char);
        assert_eq!(RfcType::from(30), RfcType::XString);
        assert_eq!(RfcType::from(9999), RfcType::Unknown(9999));
        assert_eq!(RfcType::from(-1), RfcType::Unknown(-1));
    }
}
