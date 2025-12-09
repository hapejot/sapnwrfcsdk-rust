use log::trace;
use serde::{ser::SerializeMap, Serialize};
use log::error;

use crate::{
    error_info, field_descriptor,
    librfc::{
        RfcCreateStructure, RfcDescribeType, RfcDestroyStructure, RfcGetChars, RfcGetFieldCount,
        RfcGetFieldDescByIndex, RfcGetFieldDescByName, RfcGetInt, RfcGetInt1, RfcGetString,
        RfcGetStructure, RfcGetTable, RfcType, RFC_STRUCTURE_HANDLE, RFC_TABLE_HANDLE,
        _RFCTYPE_RFCTYPE_CHAR as RFCTYPE_RFCTYPE_CHAR,
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
    #[tracing::instrument]
    fn drop(&mut self) {
        if !self.dependent {
            let mut errorInfo = error_info();
            unsafe {
                RfcDestroyStructure(self.handle, &mut errorInfo);
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
        let n = self.fields.len();
        for idx in 0..n {
            trace!("serializing field: {idx}/{n} {}", &self.fields[idx]);
            match self.get(&self.fields[idx]) {
                Ok(value) => {
                    let field = &self.fields[idx];
                    map.serialize_entry(field, &value)?;
                }
                Err(e) => {
                    error!("failed to deserialize field {}: {}", &self.fields[idx], e);
                }
            }
        }
        trace!("serializing structure done");
        map.end()
    }
}

impl SapStructure {
    pub fn new(handle: RFC_STRUCTURE_HANDLE, dependent: bool) -> Result<Self, String> {
        let mut errorInfo = error_info();
        let mut count: cty::c_uint = 0;
        let mut fields = vec![];
        unsafe {
            let type_handle = RfcDescribeType(handle, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            let rc = RfcGetFieldCount(type_handle, &mut count, &mut errorInfo);
            if errorInfo.code != 0 || rc != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }

            for idx in 0..count {
                let mut fieldDescr = field_descriptor();
                let rc = RfcGetFieldDescByIndex(type_handle, idx, &mut fieldDescr, &mut errorInfo);
                if errorInfo.code != 0 || rc != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                let fld = String::from(&SapString::from(fieldDescr.name.as_slice()));
                fields.push(fld);
            }
        }

        Ok(Self {
            handle,
            dependent,
            fields,
        })
    }

    pub fn set<V>(&self, name: &str, value: V) -> Result<(), String>
    where
        V: Into<Value>,
    {
        let v: Value = value.into();
        match v {
            Value::String(sap_string) => set_chars(self.handle, name, sap_string)?,
            _ => todo!(),
        }
        Ok(())
    }

    pub fn get<S>(&self, name: S) -> Result<Value, String>
    where
        S: Into<String>,
    {
        let mut errorInfo = error_info();
        let mut fieldDescr = field_descriptor();
        let str_name: String = name.into();
        let sap_name = SapString::from(str_name);
        let type_handle = unsafe { RfcDescribeType(self.handle, &mut errorInfo) };
        if errorInfo.code != 0 {
            return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
        }
        let rc = unsafe {
            RfcGetFieldDescByName(
                type_handle,
                sap_name.raw_pointer(),
                &mut fieldDescr,
                &mut errorInfo,
            )
        };
        if rc != 0 {
            return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
        }
        trace!("field description: {:} uc-length: {}", fieldDescr.type_, fieldDescr.ucLength);
        assert_eq!(0, rc);
        match RfcType::from(fieldDescr.type_) {
            RfcType::Char => {
                let mut buffer = vec![0; fieldDescr.ucLength as usize + 1];
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        fieldDescr.ucLength,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Date => {
                let mut buffer = vec![0; fieldDescr.ucLength as usize + 1];
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        fieldDescr.ucLength,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Time => {
                let mut buffer = vec![0; fieldDescr.ucLength as usize + 1];
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        fieldDescr.ucLength,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Bcd => {
                let mut buffer = vec![0; fieldDescr.ucLength as usize + 3];
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Num => {
                let mut buffer = vec![0; fieldDescr.ucLength as usize + 3];
                let rc = unsafe {
                    RfcGetChars(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Int1 => {
                let mut value = 0;
                unsafe {
                    RfcGetInt1(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut value,
                        &mut errorInfo,
                    )
                };
                Ok(Value::Int(value as _))
            }
            RfcType::Int2 => {
                let mut value = 0;
                unsafe {
                    RfcGetInt(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut value,
                        &mut errorInfo,
                    )
                };
                Ok(Value::Int(value as _))
            }
            RfcType::Int => {
                let mut value = 0;
                unsafe {
                    RfcGetInt(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut value,
                        &mut errorInfo,
                    )
                };
                Ok(Value::Int(value as _))
            }
            RfcType::Structure => {
                let mut structHandle = 0 as RFC_STRUCTURE_HANDLE;
                let rc = unsafe {
                    RfcGetStructure(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut structHandle,
                        &mut errorInfo,
                    )
                };
                if rc != 0 || errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                Ok(Value::Structure(SapStructure::new(structHandle, true)?))
            }
            RfcType::String => {
                let mut buffer = vec![0; 5000];
                let rc = unsafe {
                    RfcGetString(
                        self.handle,
                        sap_name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        std::ptr::null_mut(),
                        &mut errorInfo,
                    )
                };

                if errorInfo.code != 0 {
                    let s = String::from(&SapString::from(errorInfo.message.as_slice()));
                    return Err(s);
                }
                assert_eq!(0, rc);
                Ok(Value::String(SapString::from(buffer.as_slice())))
            }
            RfcType::Table => {
                trace!("getting table for field: {:?}", sap_name);
                let mut table_handle = 0 as RFC_TABLE_HANDLE;
                let rc = unsafe {
                    RfcGetTable(
                        self.handle,
                        sap_name.raw_pointer(),
                        &mut table_handle,
                        &mut errorInfo,
                    )
                };
                if rc != 0 || errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                let t = SapTable::new(table_handle, true);
                trace!("got table: {:?}", t);
                Ok(Value::Table(t))
            }
            x => Err(format!("Unsupported field type in structure: {x:?}")),
        }
    }

    pub fn from_type(type_handle: crate::RFC_TYPE_DESC_HANDLE) -> Self {
        trace!("creating structure from type handle");
        unsafe {
            let mut errorInfo = error_info();
            let struct_handle = RfcCreateStructure(type_handle, &mut errorInfo);
            if errorInfo.code != 0 {
                panic!(
                    "Failed to create structure: {}",
                    String::from(&SapString::from(errorInfo.message.as_slice()))
                );
            }
            Self::new(struct_handle, false).unwrap()
        }
    }

    pub fn handle(&self) -> *mut crate::RFC_DATA_CONTAINER {
        self.handle
    }
}
impl std::fmt::Debug for SapStructure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut errorInfo = error_info();
        let mut count: cty::c_uint = 0;
        let mut dbg = f.debug_struct("SapStructure");
        dbg.field("handle", &self.handle);
        unsafe {
            let type_handle = RfcDescribeType(self.handle, &mut errorInfo);
            assert_eq!(errorInfo.code, 0);
            let rc = RfcGetFieldCount(type_handle, &mut count, &mut errorInfo);

            for idx in 0..count {
                let mut fieldDescr = field_descriptor();
                let rc = RfcGetFieldDescByIndex(type_handle, idx, &mut fieldDescr, &mut errorInfo);
                assert_eq!(0, rc);
                let dbg_val: Box<dyn std::fmt::Debug> = match fieldDescr.type_ {
                    crate::_RFCTYPE_RFCTYPE_CHAR => {
                        let mut buffer = vec![0; fieldDescr.ucLength as usize + 1];
                        let rc = RfcGetChars(
                            self.handle,
                            fieldDescr.name.as_ptr(),
                            buffer.as_mut_ptr(),
                            fieldDescr.ucLength,
                            &mut errorInfo,
                        );
                        assert_eq!(0, rc);
                        Box::new(String::from(&SapString::from(buffer.as_slice())))
                    }
                    _ => Box::new(String::from("<value>")),
                };
                dbg.field(
                    String::from(&SapString::from(fieldDescr.name.as_slice())).as_str(),
                    &dbg_val,
                );
            }
            assert_eq!(0, rc);
        }
        dbg.finish()
    }
}
