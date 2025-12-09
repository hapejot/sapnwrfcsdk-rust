#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use lazy_static::*;
use log::{info, trace};
use std::{fmt::Display, sync::Mutex};
use string::SapString;

use crate::librfc::{
    RfcAppendNewRow, RfcCreateStructure, RfcCreateTable, RfcGetFieldCount, RfcGetFieldDescByIndex, RfcSetChars, RfcSetStructure, RfcSetTable, RfcSetXString, RFC_DATA_CONTAINER, RFC_ERROR_INFO, RFC_FIELD_DESC, RFC_PARAMETER_DESC, RFC_TYPE_DESC_HANDLE, _RFCTYPE_RFCTYPE_CHAR, _RFCTYPE_RFCTYPE_STRING, _RFCTYPE_RFCTYPE_STRUCTURE, _RFCTYPE_RFCTYPE_TABLE, _RFC_FIELD_DESC, _RFC_TYPE_DESC_HANDLE
};

lazy_static! {
    static ref CONNECT_COUNT: Mutex<i32> = Mutex::new(0);
}

fn any_to_string<T: Display>(value: T) -> String {
    value.to_string()
}
#[allow(dead_code)]
mod librfc {
    #[repr(i32)]
    #[derive(Debug)]
    pub enum RfcType {
        Char = _RFCTYPE_RFCTYPE_CHAR,
        Date = _RFCTYPE_RFCTYPE_DATE,
        Time = _RFCTYPE_RFCTYPE_TIME,
        Byte = _RFCTYPE_RFCTYPE_BYTE,
        Float = _RFCTYPE_RFCTYPE_FLOAT,
        Int1 = _RFCTYPE_RFCTYPE_INT1,
        Int2 = _RFCTYPE_RFCTYPE_INT2,
        Int8 = _RFCTYPE_RFCTYPE_INT8,
        Bcd = _RFCTYPE_RFCTYPE_BCD,
        Num = _RFCTYPE_RFCTYPE_NUM,
        Int = _RFCTYPE_RFCTYPE_INT,        
        String = _RFCTYPE_RFCTYPE_STRING,
        Structure = _RFCTYPE_RFCTYPE_STRUCTURE,
        Table = _RFCTYPE_RFCTYPE_TABLE,
        XString = _RFCTYPE_RFCTYPE_XSTRING,   
    }

    impl From<i32> for RfcType {
        fn from(value: i32) -> Self {
            unsafe { std::mem::transmute(value) }
        }
    }
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub mod rfc_param;
mod string;

pub mod connection;

mod function;

mod structure;
mod table;

pub mod value;

/// Creates a new `RFC_FIELD_DESC` with default values.
/// This function initializes an `RFC_FIELD_DESC` structure with zeroed fields.
/// It sets the name to a zeroed `SapString`, type to 0, and all other fields to zero or null pointers.
/// This is useful for creating a field descriptor that can be used as a template or placeholder
/// before being filled with actual data.
fn field_descriptor() -> _RFC_FIELD_DESC {
    let fieldDescr = RFC_FIELD_DESC {
        name: zero(),
        type_: 0,
        nucLength: 0,
        nucOffset: 0,
        ucLength: 0,
        ucOffset: 0,
        decimals: 0,
        typeDescHandle: 0 as RFC_TYPE_DESC_HANDLE,
        extendedDescription: 0 as *mut cty::c_void,
    };
    fieldDescr
}

/// Creates a zero-initialized `RFC_PARAMETER_DESC` structure.
/// This function initializes the `RFC_PARAMETER_DESC` structure with default values.
/// The structure contains fields for parameter name, type, direction, length, decimals,
/// type description handle, default value, parameter text, optional flag, and extended description.
fn parameter_description() -> RFC_PARAMETER_DESC {
    let paramDesc = RFC_PARAMETER_DESC {
        name: zero(),
        type_: 0,
        direction: 0,
        nucLength: 0,
        ucLength: 0,
        decimals: 0,
        typeDescHandle: 0 as RFC_TYPE_DESC_HANDLE,
        defaultValue: zero(),
        parameterText: zero(),
        optional: 0,
        extendedDescription: 0 as *mut cty::c_void,
    };
    paramDesc
}

/// Creates a zero-initialized array of `u16` with the specified size.
/// This function returns an array of `u16` with all elements set to zero.
fn zero<const N: usize>() -> [u16; N] {
    [0; N]
}

/// Creates a new `RFC_ERROR_INFO` structure with all fields initialized to zero.
/// This function initializes the `RFC_ERROR_INFO` structure with default values.
/// The structure contains fields for ABAP message class, number, type, and various message variables.
/// It also includes fields for error code, group, key, and message.
/// # Returns
/// * `RFC_ERROR_INFO` - A new instance of the `RFC_ERROR_INFO` structure.
fn error_info() -> RFC_ERROR_INFO {
    let z = RFC_ERROR_INFO {
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
    };
    z
}

/// Dumps the memory content of a pointer to a 16-bit character array.
/// This function takes a pointer to a 16-bit character array and prints the first 16 characters
/// in hexadecimal format for debugging purposes.
#[allow(dead_code)]
fn dump_memory(name_ptr: *const u16) {
    trace!("name: {:p}", name_ptr);
    for i in 0..16 as usize {
        unsafe {
            trace!(" {:02x}", *(name_ptr.add(i)));
        }
    }
}

/// Sets a character field in the RFC data container from a `SapString`.
/// This function takes a pointer to the RFC data container, a field name, and a `SapString` value.
/// It uses the `RfcSetChars` function to set the value in the RFC data container.
/// If the operation fails, it returns an error message.
/// # Arguments
/// * `cont` - A pointer to the RFC data container.
/// * `name` - The name of the field to set.
/// * `value` - The `SapString` value to set in the field.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the `RfcSetChars` function fails, containing the error message from the SAP system.       
///
fn set_chars(cont: *mut RFC_DATA_CONTAINER, name: &str, value: SapString) -> Result<(), String> {
    let mut errorInfo = error_info();
    let str_name = SapString::from(name);

    unsafe {
        let rc = RfcSetChars(
            cont,
            str_name.raw_pointer(),
            value.raw_pointer(),
            value.len() as u32,
            &mut errorInfo,
        );
        trace!("set value for {}: {:?} -> {}", name, value, rc);
        if rc != 0 {
            let x = SapString::from(errorInfo.message.as_slice());
            return Err(String::from(&x));
        }
        assert_eq!(0, rc);
    }
    Ok(())
}


/// Sets a xstring field in the RFC data container from a string.
/// This function takes a pointer to the RFC data container, a field name, and a string value.
/// It converts the string to a `SapString` and uses the `RfcSetChars` function to set the value.
/// If the operation fails, it returns an error message.
/// # Arguments
/// * `cont` - A pointer to the RFC data container.     
/// * `name` - The name of the field to set.
/// * `value` - The string value to set in the field.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the `RfcSetChars` function fails, containing the error message from the SAP system.
pub fn set_xstring_from_str(
    cont: *mut crate::RFC_DATA_CONTAINER,
    name: &str,
    value: &str,
) -> Result<(), String> {
    let mut errorInfo = error_info();
    let str_name = SapString::from(name);
    let value = value.as_bytes();
    unsafe {
        let rc = RfcSetXString(
            cont,
            str_name.raw_pointer(),
            value.as_ptr(),
            value.len() as u32,
            &mut errorInfo,
        );
        trace!("set value for {}: {:?} -> {}", name, value, rc);
        if rc != 0 {
            let x = SapString::from(errorInfo.message.as_slice());
            return Err(String::from(&x));
        }
        assert_eq!(0, rc);
    }
    Ok(())
}

/// Sets a character field in the RFC data container from a string.
/// This function takes a pointer to the RFC data container, a field name, and a string value.
/// It converts the string to a `SapString` and uses the `RfcSetChars` function to set the value.
/// If the operation fails, it returns an error message.
/// # Arguments
/// * `cont` - A pointer to the RFC data container.     
/// * `name` - The name of the field to set.
/// * `value` - The string value to set in the field.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the `RfcSetChars` function fails, containing the error message from the SAP system.
fn set_chars_from_str(
    cont: *mut crate::RFC_DATA_CONTAINER,
    name: &str,
    value: &str,
) -> Result<(), String> {
    let mut errorInfo = error_info();
    let str_name = SapString::from(name);
    let value = SapString::from(value);
    unsafe {
        let rc = RfcSetChars(
            cont,
            str_name.raw_pointer(),
            value.raw_pointer(),
            value.len() as u32,
            &mut errorInfo,
        );
        trace!("set value for {}: {:?} -> {}", name, value, rc);
        if rc != 0 {
            let x = SapString::from(errorInfo.message.as_slice());
            return Err(String::from(&x));
        }
        assert_eq!(0, rc);
    }
    Ok(())
}

/// Sets a table in the RFC data container from a JSON array.
/// This function iterates over the provided JSON array and sets each field in the table according to its type handle.
/// It supports fields of type CHAR, STRING, and STRUCTURE.
/// If a field is of type STRUCTURE, it recursively calls itself to set the structure from the type handle.
/// If a field is of type TABLE, it currently does nothing, as handling for nested tables is not implemented.
/// # Arguments
/// * `cont` - A pointer to the RFC data container.
/// * `name` - The name of the table to set.
/// * `type_handle` - The type handle of the table.
/// * `values` - A JSON array containing the values to set in the table.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the field type is unsupported or if setting the table fails.
fn set_structure_from_type_handle(
    cont: *mut crate::RFC_DATA_CONTAINER,
    name: &str,
    type_handle: crate::RFC_TYPE_DESC_HANDLE,
    value: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let mut errorInfo = error_info();
    let structure_handle = unsafe { RfcCreateStructure(type_handle, &mut errorInfo) };
    fill_structure(structure_handle, type_handle, value)?;

    let name_sap = SapString::from(name);
    let rc = unsafe {
        RfcSetStructure(
            cont,
            name_sap.raw_pointer(),
            structure_handle,
            &mut errorInfo,
        )
    };
    if rc != 0 {
        let x = SapString::from(errorInfo.message.as_slice());
        trace!("sap string created");
        return Err(String::from(&x));
    }
    assert_eq!(0, rc);

    Ok(())
}

/// Fills an RFC data container structure from a JSON map.
/// This function iterates over the fields in the structure type handle and sets the values in the
/// RFC data container according to the field type.
/// It supports fields of type CHAR, STRING, STRUCTURE, and TABLE.
/// If a field is of type STRUCTURE, it recursively calls itself to fill the structure from the type handle.
/// If a field is of type TABLE, it calls `set_table_from_type_handle` to set the table values.
/// # Arguments
/// * `row_handle` - A pointer to the RFC data container.
/// * `row_type_handle` - A pointer to the type handle of the structure.            
/// * `values_map` - A map containing the field names and their corresponding JSON values.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the field type is unsupported or if setting the structure fails.  
/// # Note
/// * This function assumes that the `row_handle` and `row_type_handle` are valid pointers to an RFC data container and its type description, respectively.
/// * It logs the processing of each field index and the expected type for each field.
/// * It handles different JSON value types (string, object, array) according to the field type.
/// * It uses `set_chars_from_str`, `set_structure_from_type_handle`, and `set_table_from_type_handle` to set the values in the RFC data container.
/// * It logs warnings if the expected type does not match the provided JSON value type.
fn fill_structure(
    row_handle: *mut RFC_DATA_CONTAINER,
    row_type_handle: *mut _RFC_TYPE_DESC_HANDLE,
    values_map: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let mut errorInfo = error_info();
    let mut count = 0;
    unsafe {
        RfcGetFieldCount(row_type_handle, &mut count, &mut errorInfo);
    }

    Ok(for idx in 0..count {
        info!("Processing field index: {}", idx);
        let mut fieldDescr = field_descriptor();
        unsafe {
            RfcGetFieldDescByIndex(row_type_handle, idx, &mut fieldDescr, &mut errorInfo);
        }
        let name = SapString::new(&fieldDescr.name);
        let name = String::from(&name);
        if let Some(v) = values_map.get(&name) {
            match fieldDescr.type_ {
                _RFCTYPE_RFCTYPE_CHAR => {
                    if let serde_json::Value::String(v) = v {
                        set_chars_from_str(row_handle, name.as_str(), v.as_str())?;
                    } else {
                        info!("Expected string for field: {}, got {:?}", name, v);
                    }
                }
                _RFCTYPE_RFCTYPE_STRUCTURE => {
                    if let serde_json::Value::Object(obj) = v {
                        set_structure_from_type_handle(
                            row_handle,
                            name.as_str(),
                            fieldDescr.typeDescHandle,
                            &obj.clone(),
                        )?;
                    } else {
                        info!("Expected structure for field: {}, got {:?}", name, v);
                    }
                }
                _RFCTYPE_RFCTYPE_STRING => {
                    if let serde_json::Value::String(v) = v {
                        set_chars_from_str(row_handle, name.as_str(), v.as_str())?;
                    } else {
                        info!("Expected string for field: {}, got {:?}", name, v);
                    }
                }
                _RFCTYPE_RFCTYPE_TABLE => {
                    if let serde_json::Value::Array(arr) = v {
                        set_table_from_type_handle(
                            row_handle,
                            name.as_str(),
                            fieldDescr.typeDescHandle,
                            arr,
                        )?;
                    } else {
                        info!("Expected array for field: {}, got {:?}", name, v);
                    }
                }
                _ => todo!("Unsupported field type: {}", fieldDescr.type_),
            }
        }
    })
}

/// Sets a table in the RFC data container from a JSON array.
/// This function iterates over the provided JSON array and sets each field in the table according to its type handle.
/// It supports fields of type CHAR, STRING, and STRUCTURE.
/// If a field is of type STRUCTURE, it recursively calls itself to set the structure from the type handle.
/// If a field is of type TABLE, it currently does nothing, as handling for nested tables is not implemented.
/// # Arguments
/// * `cont` - A pointer to the RFC data container.
/// * `name` - The name of the table to set.
/// * `type_handle` - The type handle of the table.     
/// * `value` - A slice of JSON values representing the table rows.
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) on success, or an error message on failure.
/// # Errors
/// * Returns an error if the field type is unsupported or if setting the structure fails.
fn set_table_from_type_handle(
    cont: *mut RFC_DATA_CONTAINER,
    name: &str,
    type_handle: RFC_TYPE_DESC_HANDLE,
    value: &[serde_json::Value],
) -> Result<(), String> {
    let mut errorInfo = error_info();
    let table_handle = unsafe { RfcCreateTable(type_handle, &mut errorInfo) };

    for v in value {
        if let serde_json::Value::Object(obj) = v {
            let row_handle = unsafe { RfcAppendNewRow(table_handle, &mut errorInfo) };
            fill_structure(row_handle, type_handle, obj)?;
        } else {
            info!("Expected object for field: {}, got {:?}", name, v);
        }
    }

    info!("Setting table: {}", name);
    let name_sap = SapString::from(name);
    let rc = unsafe { RfcSetTable(cont, name_sap.raw_pointer(), table_handle, &mut errorInfo) };
    if rc != 0 {
        let x = SapString::from(errorInfo.message.as_slice());
        return Err(String::from(&x));
    }
    assert_eq!(0, rc);
    Ok(())
}
