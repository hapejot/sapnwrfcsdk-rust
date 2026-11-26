use log::trace;
use serde::{ser::SerializeMap, Serialize};

use crate::{
    check_ptr, check_rc, error::RfcError, error_info, field_descriptor, RfcResult,
    librfc::{
        RfcCreateStructure, RfcDescribeType, RfcDestroyStructure, RfcGetBytes, RfcGetChars,
        RfcGetFieldCount, RfcGetFieldDescByIndex, RfcGetFieldDescByName, RfcGetInt, RfcGetInt1,
        RfcGetString, RfcGetStructure, RfcGetTable, RfcType, RFC_STRUCTURE_HANDLE,
        RFC_TABLE_HANDLE, RFC_TYPE_DESC_HANDLE,
    },
    set_chars,
    string::SapString,
    table::SapTable,
    value::Value,
};

pub struct SapStructure {
    handle: RFC_STRUCTURE_HANDLE,
    dependent: bool,
    fields: Vec<String>,
}

impl Drop for SapStructure {
    fn drop(&mut self) {
        if !self.dependent {
            let mut ei = error_info();
            unsafe {
                RfcDestroyStructure(self.handle, &mut ei);
            }
        }
    }
}

impl Serialize for SapStructure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        trace!("serializing structure");
        let mut map = serializer.serialize_map(Some(self.fields.len()))?;
        for field in &self.fields {
            trace!("serializing field: {field}");
            let value = self.get(field.as_str()).map_err(serde::ser::Error::custom)?;
            map.serialize_entry(field, &value)?;
        }
        trace!("serializing structure done");
        map.end()
    }
}

impl SapStructure {
    pub fn new(handle: RFC_STRUCTURE_HANDLE, dependent: bool) -> RfcResult<Self> {
        let mut ei = error_info();
        let mut count: cty::c_uint = 0;
        let mut fields = vec![];

        let type_handle = unsafe { RfcDescribeType(handle, &mut ei) };
        check_ptr(type_handle, &ei, "RfcDescribeType")?;
        let rc = unsafe { RfcGetFieldCount(type_handle, &mut count, &mut ei) };
        check_rc(rc, &ei)?;

        for idx in 0..count {
            let mut field_descr = field_descriptor();
            let mut ei = error_info();
            let rc = unsafe { RfcGetFieldDescByIndex(type_handle, idx, &mut field_descr, &mut ei) };
            check_rc(rc, &ei)?;
            fields.push(String::from(&SapString::from(field_descr.name.as_slice())));
        }

        Ok(Self {
            handle,
            dependent,
            fields,
        })
    }

    pub fn set<V>(&self, name: &str, value: V) -> RfcResult<()>
    where
        V: Into<Value>,
    {
        match value.into() {
            Value::String(sap_string) => set_chars(self.handle, name, sap_string),
            _ => Err(RfcError::InvalidInput(format!(
                "Nur Strings können in Feld {name} gesetzt werden"
            ))),
        }
    }

    pub fn get<S>(&self, name: S) -> RfcResult<Value>
    where
        S: Into<String>,
    {
        let str_name: String = name.into();
        let sap_name = SapString::from(str_name.as_str());

        let mut ei = error_info();
        let type_handle = unsafe { RfcDescribeType(self.handle, &mut ei) };
        check_ptr(type_handle, &ei, "RfcDescribeType")?;

        let mut field_descr = field_descriptor();
        let mut ei = error_info();
        let rc = unsafe {
            RfcGetFieldDescByName(type_handle, sap_name.raw_pointer(), &mut field_descr, &mut ei)
        };
        check_rc(rc, &ei)?;
        trace!(
            "field description: {:} uc-length: {}",
            field_descr.type_,
            field_descr.ucLength
        );

        match RfcType::from(field_descr.type_) {
            RfcType::Char | RfcType::Date | RfcType::Time => {
                let mut buffer = vec![0; field_descr.ucLength as usize + 1];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        field_descr.ucLength,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Bcd | RfcType::Num => {
                let mut buffer = vec![0; field_descr.ucLength as usize + 3];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Int1 => {
                let mut value = 0;
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetInt1(self.handle, sap_name.raw_pointer(), &mut value, &mut ei)
                };
                check_rc(rc, &ei)?;
                Ok(Value::Int(value as _))
            }
            RfcType::Int2 | RfcType::Int => {
                let mut value = 0;
                let mut ei = error_info();
                let rc = unsafe { RfcGetInt(self.handle, sap_name.raw_pointer(), &mut value, &mut ei) };
                check_rc(rc, &ei)?;
                Ok(Value::Int(value as _))
            }
            RfcType::Structure => {
                let mut struct_handle: RFC_STRUCTURE_HANDLE = std::ptr::null_mut();
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetStructure(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut struct_handle,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                Ok(Value::Structure(SapStructure::new(struct_handle, true)?))
            }
            RfcType::String => {
                // Zweistufig: erst die Länge ermitteln, dann mit passendem Puffer lesen.
                // Der Rückgabewert der Längenabfrage wird ignoriert, weil ein zu kleiner Puffer
                // dort gewollt ist; entscheidend ist `strlen`. Das eigentliche Lesen wird
                // mit frischer Fehlerinfo ausgeführt und geprüft.
                let mut strlen = 0u32;
                let mut probe = [0; 5];
                let mut ei = error_info();
                let _ = unsafe {
                    RfcGetString(
                        self.handle,
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
                        self.handle,
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
            RfcType::Table => {
                trace!("getting table for field: {:?}", sap_name);
                let mut table_handle: RFC_TABLE_HANDLE = std::ptr::null_mut();
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetTable(self.handle, sap_name.raw_pointer(), &mut table_handle, &mut ei)
                };
                check_rc(rc, &ei)?;
                let t = SapTable::new(table_handle, true);
                trace!("got table: {:?}", t);
                Ok(Value::Table(t))
            }
            RfcType::Byte => {
                let mut buffer = vec![0u8; field_descr.ucLength as usize + 3];
                let mut ei = error_info();
                let rc = unsafe {
                    RfcGetBytes(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut ei,
                    )
                };
                check_rc(rc, &ei)?;
                let hex: String = buffer.iter().map(|x| format!("{x:02x}")).collect();
                Ok(Value::String(SapString::from(hex)))
            }
            other => Err(RfcError::UnsupportedType {
                name: str_name,
                type_code: match other {
                    RfcType::Unknown(code) => code,
                    _ => field_descr.type_,
                },
            }),
        }
    }

    /// Erzeugt eine eigenständige Struktur (wird beim Drop zerstört) zu einem Typ-Handle.
    pub fn from_type(type_handle: RFC_TYPE_DESC_HANDLE) -> RfcResult<Self> {
        trace!("creating structure from type handle");
        let mut ei = error_info();
        let struct_handle = unsafe { RfcCreateStructure(type_handle, &mut ei) };
        let struct_handle = check_ptr(struct_handle, &ei, "RfcCreateStructure")?;
        Self::new(struct_handle, false)
    }

    pub fn handle(&self) -> *mut crate::RFC_DATA_CONTAINER {
        self.handle
    }
}

impl std::fmt::Debug for SapStructure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("SapStructure");
        dbg.field("handle", &self.handle);
        for name in &self.fields {
            match self.get(name.as_str()) {
                Ok(value) => dbg.field(name.as_str(), &value),
                Err(e) => dbg.field(name.as_str(), &format!("<error: {e}>")),
            };
        }
        dbg.finish()
    }
}
