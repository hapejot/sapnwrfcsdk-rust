use core::error;

use log::{error, info, trace};
use serde::{
    ser::{self, SerializeMap},
    Serialize,
};

use crate::{
    error_info,
    librfc::{
        RfcDestroyFunction, RfcDestroyFunctionDesc, RfcGetChars, RfcGetInt, RfcGetParameterCount,
        RfcGetParameterDescByIndex, RfcGetParameterDescByName, RfcGetString, RfcGetStringLength,
        RfcGetStructure, RfcGetTable, RfcGetXString, RfcInvoke, RfcType, RFC_CONNECTION_HANDLE,
        RFC_DATA_CONTAINER, RFC_FUNCTION_DESC_HANDLE, RFC_FUNCTION_HANDLE, RFC_INT,
        RFC_STRUCTURE_HANDLE, RFC_TABLE_HANDLE, RFC_TYPE_DESC_HANDLE,
        _RFCTYPE_RFCTYPE_CHAR as RFCTYPE_RFCTYPE_CHAR, _RFCTYPE_RFCTYPE_INT as RFCTYPE_RFCTYPE_INT,
        _RFCTYPE_RFCTYPE_STRING as RFCTYPE_RFCTYPE_STRING,
        _RFCTYPE_RFCTYPE_STRUCTURE as RFCTYPE_RFCTYPE_STRUCTURE,
        _RFCTYPE_RFCTYPE_TABLE as RFCTYPE_RFCTYPE_TABLE,
        _RFC_DIRECTION_RFC_CHANGING as RFC_DIRECTION_RFC_CHANGING,
        _RFC_DIRECTION_RFC_EXPORT as RFC_DIRECTION_RFC_EXPORT,
        _RFC_DIRECTION_RFC_IMPORT as RFC_DIRECTION_RFC_IMPORT,
        _RFC_DIRECTION_RFC_TABLES as RFC_DIRECTION_RFC_TABLES,
    },
    parameter_description, set_chars, set_chars_from_str, set_structure_from_type_handle,
    set_table_from_type_handle, set_xstring_from_str,
    string::SapString,
    structure::SapStructure,
    table,
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
            map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?
            // match p.direction {
            //     ParameterDirection::Import => {
            //         map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?
            //     }
            //     ParameterDirection::Export => {
            //         map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?
            //     }
            //     ParameterDirection::Changing => {
            //         map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?
            //     }
            //     ParameterDirection::Table => {
            //         map.serialize_entry(&p.name, &self.get(&p.name).map_err(ser::Error::custom)?)?
            //     }
            // }
        }
        map.end()
    }
}

impl Function {
    pub fn new(
        cn: RFC_CONNECTION_HANDLE,
        fh: RFC_FUNCTION_HANDLE,
        fd: RFC_FUNCTION_DESC_HANDLE,
    ) -> Result<Self, String> {
        let mut errorInfo = error_info();
        let mut params = vec![];
        let mut count: cty::c_uint = 0;
        let rc = unsafe { RfcGetParameterCount(fd, &mut count as *mut u32, &mut errorInfo) };
        if errorInfo.code != 0 {
            return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
        }
        assert_eq!(rc, 0);
        trace!("param count: {count}");
        for i in 0..count {
            let mut paramDesc = parameter_description();
            let rc = unsafe { RfcGetParameterDescByIndex(fd, i, &mut paramDesc, &mut errorInfo) };
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }

            assert_eq!(rc, 0);

            let s = paramDesc.name.as_slice();
            let direction = match paramDesc.direction as i32 {
                RFC_DIRECTION_RFC_IMPORT => ParameterDirection::Import,
                RFC_DIRECTION_RFC_EXPORT => ParameterDirection::Export,
                RFC_DIRECTION_RFC_CHANGING => ParameterDirection::Changing,
                RFC_DIRECTION_RFC_TABLES => ParameterDirection::Table,
                _ => panic!("Unknown parameter direction: {}", paramDesc.direction),
            };
            let length = paramDesc.ucLength as i32;
            let decimals = paramDesc.decimals as i32;
            let datatype = paramDesc.type_;
            let name = String::from(&SapString::from(s));
            params.push(ParameterDescription {
                name,
                datatype,
                direction,
                length,
                decimals,
                type_desc_handle: paramDesc.typeDescHandle,
            })
        }

        Ok(Self { cn, fh, fd, params })
    }

    pub fn execute(&self) -> Result<(), String> {
        trace!("Executing function");
        let mut errorInfo = error_info();
        unsafe {
            let rc = RfcInvoke(self.cn, self.fh as *mut RFC_DATA_CONTAINER, &mut errorInfo);
            if errorInfo.code != 0 {
                error!(
                    "{} {} {} {} {}", errorInfo.code,
                    String::from(&SapString::from(errorInfo.abapMsgClass.as_slice())),
                    String::from(&SapString::from(errorInfo.abapMsgType.as_slice())),
                    String::from(&SapString::from(errorInfo.abapMsgNumber.as_slice())),
                    String::from(&SapString::from(errorInfo.message.as_slice()))
                );
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_eq!(0, rc);
        }
        trace!("Executing function done");
        Ok(())
    }

    pub fn set<V>(&self, name: &str, value: V) -> Result<(), String>
    where
        V: Into<Value>,
    {
        let v: Value = value.into();
        match v {
            Value::String(sap_string) => set_chars(self.fh, name, sap_string)?,
            _ => todo!(),
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<Value, String> {
        trace!("Getting value for parameter: {}", name);
        let mut paramDesc = parameter_description();
        let mut errorInfo = error_info();
        let name = SapString::from(name);
        unsafe {
            RfcGetParameterDescByName(self.fd, name.raw_pointer(), &mut paramDesc, &mut errorInfo);
        }
        if errorInfo.code != 0 {
            return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
        }
        let v = match RfcType::from(paramDesc.type_) {
            RfcType::Int => {
                let mut value: RFC_INT = 0;
                unsafe { RfcGetInt(self.fh, name.raw_pointer(), &mut value, &mut errorInfo) };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                Value::Int(value as i64)
            }
            RfcType::Structure => {
                let mut structHandle: RFC_STRUCTURE_HANDLE = 0 as RFC_STRUCTURE_HANDLE;
                let rc = unsafe {
                    RfcGetStructure(
                        self.fh,
                        name.raw_pointer(),
                        &mut structHandle,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                trace!("structure handle: {:p}", structHandle);
                Value::Structure(SapStructure::new(structHandle, true).unwrap())
            }
            RfcType::Table => {
                let mut handle: RFC_TABLE_HANDLE = 0 as RFC_TABLE_HANDLE;
                let rc = unsafe {
                    RfcGetTable(self.fh, name.raw_pointer(), &mut handle, &mut errorInfo)
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                trace!("table handle: {:p}", handle);
                Value::Table(table::SapTable::new(handle, true))
            }
            RfcType::String => {
                trace!("getting string value for {:?}", name);
                let mut strlen = 0u32;
                let mut buf0 = [0; 5];
                let _rc = unsafe {
                    RfcGetString(
                        self.fh,
                        name.raw_pointer(),
                        buf0.as_mut_ptr(),
                        buf0.len() as u32,
                        &mut strlen,
                        &mut errorInfo,
                    )
                };
                let mut buffer = vec![0; strlen as usize + 1];

                let rc = unsafe {
                    RfcGetString(
                        self.fh,
                        name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        std::ptr::null_mut(),
                        &mut errorInfo,
                    )
                };

                if errorInfo.code != 0 {
                    let s = String::from(&SapString::from(errorInfo.message.as_slice()));
                    error!("RfcGetString failed: {}", &s);
                    return Err(s);
                }
                assert_eq!(0, rc);
                Value::String(SapString::from(buffer.as_slice()))
            }
            RfcType::Char => {
                trace!("getting char value for {:?}", name);
                let mut buffer = vec![0; paramDesc.ucLength as usize + 1];
                let rc = unsafe {
                    RfcGetChars(
                        self.fh,
                        name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        paramDesc.ucLength as u32,
                        &mut errorInfo,
                    )
                };
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                Value::String(SapString::from(buffer.as_slice()))
            }
            RfcType::XString => {
                trace!("getting xstring value for {:?}", name);
                let mut n: u32 = 0;
                let rc = unsafe {
                    RfcGetStringLength(self.fh, name.raw_pointer(), &mut n, &mut errorInfo)
                };
                let mut buffer = vec![0; n as usize];
                let mut xstring_length: u32 = 0;
                let rc = unsafe {
                    RfcGetXString(
                        self.fh,
                        name.raw_pointer(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut xstring_length,
                        &mut errorInfo,
                    )
                };

                if errorInfo.code != 0 {
                    let s = String::from(&SapString::from(errorInfo.message.as_slice()));
                    error!("RfcGetXString failed: {}", &s);
                    return Err(s);
                }
                assert_eq!(0, rc);
                let s = unsafe {
                    String::from_utf8_unchecked(buffer[0..xstring_length as usize].to_vec())
                };
                Value::String(SapString::from(s.as_str()))
            }
            _ => todo!(
                "Unsupported parameter type: {} for parameter {:?}",
                paramDesc.type_,
                name
            ),
        };
        trace!("get value done");
        Ok(v)
    }

    pub fn set_parameters(&self, p: &serde_json::Value) -> Result<(), String> {
        info!("settings parameters");
        match p {
            serde_json::Value::Object(map) => {
                for (key, value) in map.iter() {
                    info!("Setting parameter: {} = {:?}", key, value);
                    let name = key.as_str();
                    let sap_name = SapString::from(name);
                    let mut paramDesc = parameter_description();
                    let mut errorInfo = error_info();
                    unsafe {
                        RfcGetParameterDescByName(
                            self.fd,
                            sap_name.raw_pointer(),
                            &mut paramDesc,
                            &mut errorInfo,
                        )
                    };
                    let typ = RfcType::from(paramDesc.type_);
                    match (value, typ) {
                        (serde_json::Value::String(s), RfcType::String) => {
                            set_chars_from_str(self.fh, name, s.as_str())?
                        }
                        (serde_json::Value::String(s), RfcType::XString) => {
                            set_xstring_from_str(self.fh, name, s.as_str())?
                        }
                        (serde_json::Value::String(s), RfcType::Char) => {
                            set_chars_from_str(self.fh, name, s.as_str())?
                        }
                        (serde_json::Value::Object(o), RfcType::Structure) => {
                            set_structure_from_type_handle(
                                self.fh,
                                name,
                                paramDesc.typeDescHandle,
                                o,
                            )?
                        }
                        (serde_json::Value::Array(a), RfcType::Table) => {
                            set_table_from_type_handle(self.fh, name, paramDesc.typeDescHandle, a)?
                        }
                        (value, typ) => {
                            return Err(format!(
                                "Unsupported parameter type combination: {:?}, {:?}",
                                value, typ
                            ))
                        }
                    }
                }
            }
            _ => return Err(format!("Unsupported parameter type: {}", p)),
        }
        trace!("set parameters done");
        Ok(())
    }
}
impl Drop for Function {
    fn drop(&mut self) {
        let mut errorInfo1 = error_info();
        let mut errorInfo2 = error_info();
        unsafe {
            RfcDestroyFunction(self.fh, &mut errorInfo2); // destry funtion handle first
                                                          // RfcDestroyFunctionDesc(self.fd, &mut errorInfo1);
        }
        trace!("drop function done");
    }
}
