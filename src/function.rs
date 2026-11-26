use log::{debug, error, trace};
use serde::{
    ser::{self, SerializeMap},
    Serialize,
};

use crate::{
    check_rc, error_info, error::RfcError, json_kind, RfcResult,
    librfc::{
        RfcDestroyFunction, RfcGetBytes, RfcGetChars, RfcGetDate, RfcGetInt,
        RfcGetParameterCount, RfcGetParameterDescByIndex, RfcGetParameterDescByName,
        RfcGetString, RfcGetStringLength, RfcGetStructure, RfcGetTable, RfcGetXString, RfcInvoke,
        RfcType, RFC_CHAR, RFC_CONNECTION_HANDLE, RFC_FUNCTION_DESC_HANDLE,
        RFC_FUNCTION_HANDLE, RFC_INT, RFC_STRUCTURE_HANDLE, RFC_TABLE_HANDLE, RFC_TYPE_DESC_HANDLE,
        _RFC_DIRECTION_RFC_CHANGING as RFC_DIRECTION_RFC_CHANGING,
        _RFC_DIRECTION_RFC_EXPORT as RFC_DIRECTION_RFC_EXPORT,
        _RFC_DIRECTION_RFC_IMPORT as RFC_DIRECTION_RFC_IMPORT,
        _RFC_DIRECTION_RFC_TABLES as RFC_DIRECTION_RFC_TABLES,
    },
    parameter_description, set_chars, set_chars_from_str, set_structure_from_type_handle,
    set_table_from_type_handle, set_xstring_from_str,
    string::SapString,
    structure::SapStructure,
    table::SapTable,
    value::Value,
};

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ParameterDirection {
    Import,
    Export,
    Changing,
    Table,
}

#[allow(dead_code)]
pub struct ParameterDescription {
    pub name: String,
    pub datatype: i32,
    pub direction: ParameterDirection,
    pub length: i32,
    pub decimals: i32,
    pub type_desc_handle: RFC_TYPE_DESC_HANDLE,
}

pub struct Function {
    cn: RFC_CONNECTION_HANDLE,
    fh: RFC_FUNCTION_HANDLE,
    fd: RFC_FUNCTION_DESC_HANDLE,
    params: Vec<ParameterDescription>,
}

impl Serialize for Function {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for p in self.params.iter() {
            map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?;
        }
        map.end()
    }
}

/// Konvertiert XSTRING-Bytes: gültiges UTF-8 wird als Text geliefert, Binärdaten als Hex.
fn xstring_to_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => e.into_bytes().iter().map(|b| format!("{b:02x}")).collect(),
    }
}

/// Erwarteter JSON-Typ für einen SAP-Typ, der über `set_parameters` gesetzt werden kann.
/// `None` bedeutet: dieser SAP-Typ wird über JSON nicht unterstützt.
fn expected_json_kind(typ: RfcType) -> Option<&'static str> {
    match typ {
        RfcType::String | RfcType::Char | RfcType::XString => Some("string"),
        RfcType::Structure => Some("object"),
        RfcType::Table => Some("array"),
        _ => None,
    }
}

impl Function {
    pub(crate) fn new(
        cn: RFC_CONNECTION_HANDLE,
        fh: RFC_FUNCTION_HANDLE,
        fd: RFC_FUNCTION_DESC_HANDLE,
    ) -> RfcResult<Self> {
        let mut ei = error_info();
        let mut count: cty::c_uint = 0;
        let rc = unsafe { RfcGetParameterCount(fd, &mut count, &mut ei) };
        check_rc(rc, &ei)?;
        trace!("param count: {count}");

        let mut params = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut param_desc = parameter_description();
            let mut ei = error_info();
            let rc = unsafe { RfcGetParameterDescByIndex(fd, i, &mut param_desc, &mut ei) };
            check_rc(rc, &ei)?;

            let name = String::from(&SapString::from(param_desc.name.as_slice()));
            let direction = match param_desc.direction as i32 {
                RFC_DIRECTION_RFC_IMPORT => ParameterDirection::Import,
                RFC_DIRECTION_RFC_EXPORT => ParameterDirection::Export,
                RFC_DIRECTION_RFC_CHANGING => ParameterDirection::Changing,
                RFC_DIRECTION_RFC_TABLES => ParameterDirection::Table,
                other => {
                    return Err(RfcError::InvalidInput(format!(
                        "Unbekannte Parameterrichtung {other} bei Parameter {name}"
                    )))
                }
            };
            params.push(ParameterDescription {
                name,
                datatype: param_desc.type_,
                direction,
                length: param_desc.ucLength as i32,
                decimals: param_desc.decimals as i32,
                type_desc_handle: param_desc.typeDescHandle,
            })
        }

        Ok(Self { cn, fh, fd, params })
    }

    /// Führt die Funktion aus.
    pub fn execute(&self) -> RfcResult<()> {
        trace!("Executing function");
        let mut ei = error_info();
        let rc = unsafe { RfcInvoke(self.cn, self.fh, &mut ei) };
        if let Err(e) = check_rc(rc, &ei) {
            error!("RFC-Aufruf fehlgeschlagen: {e}");
            return Err(e);
        }
        trace!("Executing function done");
        Ok(())
    }

    /// Setzt einen CHAR-/STRING-Parameter.
    pub fn set<V>(&self, name: &str, value: V) -> RfcResult<()>
    where
        V: Into<Value>,
    {
        match value.into() {
            Value::String(sap_string) => set_chars(self.fh, name, sap_string),
            _ => Err(RfcError::InvalidInput(format!(
                "Nur Strings können als Parameter {name} gesetzt werden"
            ))),
        }
    }

    /// Liest einen Parameter.
    pub fn get(&self, name: &str) -> RfcResult<Value> {
        trace!("Getting value for parameter: {}", name);
        let sap_name = SapString::from(name);

        let mut param_desc = parameter_description();
        let mut ei = error_info();
        let rc = unsafe {
            RfcGetParameterDescByName(self.fd, sap_name.raw_pointer(), &mut param_desc, &mut ei)
        };
        check_rc(rc, &ei)?;

        let unsupported = |type_code: i32| RfcError::UnsupportedType {
            name: name.to_string(),
            type_code,
        };

        match RfcType::from(param_desc.type_) {
            RfcType::Int => {
                let mut value: RFC_INT = 0;
                let mut ei = error_info();
                let rc = unsafe { RfcGetInt(self.fh, sap_name.raw_pointer(), &mut value, &mut ei) };
                check_rc(rc, &ei)?;
                Ok(Value::Int(value as i64))
            }
            RfcType::Structure => {
                let mut struct_handle: RFC_STRUCTURE_HANDLE = std::ptr::null_mut();
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetStructure(self.fh, sap_name.raw_pointer(), &mut struct_handle, &mut ei)
                };
                check_rc(rc, &ei)?;
                trace!("structure handle: {:p}", struct_handle);
                Ok(Value::Structure(SapStructure::new(struct_handle, true)?))
            }
            RfcType::Table => {
                let mut handle: RFC_TABLE_HANDLE = std::ptr::null_mut();
                let mut ei = error_info();
                let rc =
                    unsafe { RfcGetTable(self.fh, sap_name.raw_pointer(), &mut handle, &mut ei) };
                check_rc(rc, &ei)?;
                trace!("table handle: {:p}", handle);
                Ok(Value::Table(SapTable::new(handle, true)))
            }
            RfcType::String => {
                trace!("getting string value for {:?}", sap_name);
                // Zweistufig: erst die Länge ermitteln, dann mit passendem Puffer lesen.
                // Die Längenabfrage liefert bei kleinem Puffer einen Rückgabewert, den wir
                // ignorieren; entscheidend ist `strlen`. Die eigentliche Leseoperation wird
                // mit frischer Fehlerinfo ausgeführt und geprüft.
                let mut strlen = 0u32;
                let mut probe = [0; 5];
                let mut ei = error_info();
                let _ = unsafe {
                    RfcGetString(
                        self.fh,
                        sap_name.raw_pointer(),
                        probe.as_mut_ptr(),
                        probe.len() as u32,
                        &mut strlen,
                        &mut ei,
                    )
                };

                let mut buffer = vec![0; strlen as usize + 1];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetString(
                        self.fh,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        std::ptr::null_mut(),
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Char => {
                trace!("getting char value for {:?}", sap_name);
                let mut buffer = vec![0; param_desc.ucLength as usize + 1];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetChars(
                        self.fh,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        param_desc.ucLength as u32,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::XString => {
                trace!("getting xstring value for {:?}", sap_name);
                let mut n: u32 = 0;
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetStringLength(self.fh, sap_name.raw_pointer(), &mut n, &mut ei)
                };
                check_rc(rc, &ei)?;

                let mut buffer = vec![0u8; n as usize];
                let mut xstring_length: u32 = 0;
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetXString(
                        self.fh,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut xstring_length,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                buffer.truncate(xstring_length as usize);
                Ok(Value::String(SapString::from(
                    xstring_to_string(buffer).as_str(),
                )))
            }
            RfcType::Date => {
                let mut value: [RFC_CHAR; 8] = ['0' as _; 8];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetDate(self.fh, sap_name.raw_pointer(), value.as_mut_ptr(), &mut ei)
                };
                check_rc(rc, &ei)?;
                Ok(Value::String(SapString::from(value.as_slice())))
            }
            RfcType::Byte => {
                trace!("getting byte value for {:?}", sap_name);
                let mut buffer = vec![0u8; param_desc.ucLength as usize + 1];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetBytes(
                        self.fh,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        param_desc.ucLength as u32,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                let hex: String = buffer.iter().map(|x| format!("{x:02x}")).collect();
                Ok(Value::String(SapString::from(hex)))
            }
            _ => Err(unsupported(param_desc.type_)),
        }
    }

    /// Setzt Parameter aus einem JSON-Objekt. Unbekannte Parameternamen und Typfehler sind Fehler.
    pub fn set_parameters(&self, p: &serde_json::Value) -> RfcResult<()> {
        debug!("setting parameters");
        let map = match p {
            serde_json::Value::Object(map) => map,
            other => {
                return Err(RfcError::InvalidInput(format!(
                    "Parameter müssen als JSON-Objekt angegeben werden, nicht als {}",
                    json_kind(other)
                )))
            }
        };

        for (key, value) in map.iter() {
            debug!("Setting parameter: {}", key);
            let name = key.to_uppercase();
            let sap_name = SapString::from(name.as_str());

            let mut param_desc = parameter_description();
            let mut ei = error_info();
            let rc = unsafe {
                RfcGetParameterDescByName(self.fd, sap_name.raw_pointer(), &mut param_desc, &mut ei)
            };
            if rc != crate::_RFC_RC_RFC_OK || ei.code != crate::_RFC_RC_RFC_OK {
                return Err(RfcError::UnknownParameter { name });
            }

            let typ = RfcType::from(param_desc.type_);
            match (value, typ) {
                (serde_json::Value::String(s), RfcType::String) => {
                    set_chars_from_str(self.fh, &name, s.as_str())?
                }
                (serde_json::Value::String(s), RfcType::XString) => {
                    set_xstring_from_str(self.fh, &name, s.as_str())?
                }
                (serde_json::Value::String(s), RfcType::Char) => {
                    set_chars_from_str(self.fh, &name, s.as_str())?
                }
                (serde_json::Value::Object(o), RfcType::Structure) => {
                    set_structure_from_type_handle(self.fh, &name, param_desc.typeDescHandle, o)?
                }
                (serde_json::Value::Array(a), RfcType::Table) => {
                    set_table_from_type_handle(self.fh, &name, param_desc.typeDescHandle, a)?
                }
                (value, typ) => match expected_json_kind(typ) {
                    Some(expected) => {
                        return Err(RfcError::TypeMismatch {
                            name,
                            expected,
                            found: json_kind(value),
                        })
                    }
                    None => {
                        return Err(RfcError::UnsupportedType {
                            name,
                            type_code: param_desc.type_,
                        })
                    }
                },
            }
        }
        trace!("set parameters done");
        Ok(())
    }
}

impl Drop for Function {
    fn drop(&mut self) {
        // Nur das Funktionshandle freigeben; die Function-Description gehört der Verbindung.
        let mut ei = error_info();
        unsafe {
            RfcDestroyFunction(self.fh, &mut ei);
        }
        trace!("drop function done");
    }
}
