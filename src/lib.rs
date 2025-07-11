#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use lazy_static::*;
use log::trace;
use std::{fmt::Display, sync::Mutex};

lazy_static! {
    static ref CONNECT_COUNT: Mutex<i32> = Mutex::new(0);
}

fn any_to_string<T: Display>(value: T) -> String {
    value.to_string()
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[derive(Debug, Clone)]
pub struct RfcParam {
    name: SapString,
    value: SapString,
}

impl RfcParam {
    pub fn new<S1, S2>(name: S1, value: S2) -> Self
    where
        S1: Into<SapString>,
        S2: Into<SapString>,
    {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone)]
pub struct SapString {
    vec: Vec<u16>,
}

impl SapString {
    pub fn new<V>(vec: V) -> Self
    where
        V: Into<Vec<u16>>,
    {
        Self { vec: vec.into() }
    }

    pub fn raw_pointer(&self) -> *const SAP_UC {
        self.vec.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.vec.len() - 1
    }
}

impl std::fmt::Debug for SapString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SapString({:?})", String::from(self))
    }
}

impl From<&str> for SapString {
    fn from(value: &str) -> Self {
        let mut v: Vec<u16> = Vec::new();
        ucs2::encode_with(value, |c| Ok(v.push(c))).unwrap();
        v.push(0);
        SapString::new(v)
    }
}

impl From<&SapString> for String {
    fn from(value: &SapString) -> Self {
        let mut v: Vec<u8> = Vec::new();
        let mut orig = value.vec.clone();
        while orig.len() > 0 {
            let l = orig.len() - 1;
            if orig[l] == 0 {
                orig.remove(l);
                continue;
            }
            if orig[l] == 32 {
                orig.remove(l);
                continue;
            }
            break;
        }
        trace!("ucs2 vector created with {} elements", orig.len());
        ucs2::decode_with(orig.as_slice(), |c| Ok(v.extend_from_slice(c))).unwrap();
        String::from_utf8(v).unwrap()
    }
}

impl From<&[u16]> for SapString {
    fn from(value: &[u16]) -> Self {
        trace!("from u16 slice");
        let mut v: Vec<u16> = Vec::new();
        for x in value {
            if *x > 0 as u16 {
                v.push(*x);
            }
        }
        v.push(0);
        trace!("end of string found {}", v.len());
        SapString::new(v)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(SapString::from(value))
    }
}

impl From<String> for SapString {
    fn from(value: String) -> Self {
        let mut v: Vec<u16> = Vec::new();
        ucs2::encode_with(value.as_str(), |c| Ok(v.push(c))).unwrap();
        v.push(0);
        SapString::new(v)
    }
}

pub struct Connection {
    cn: RFC_CONNECTION_HANDLE,
    params: Vec<RfcParam>,
}

pub struct Function {
    cn: RFC_CONNECTION_HANDLE,
    fh: RFC_FUNCTION_HANDLE,
    fd: RFC_FUNCTION_DESC_HANDLE,
}

pub struct SapTable {
    handle: RFC_TABLE_HANDLE,
    dependent: bool,
}

pub struct SapTableIterator {
    handle: RFC_TABLE_HANDLE,
    lines: u32,
    tabix: u32,
}

impl SapTableIterator {
    pub fn new(handle: RFC_TABLE_HANDLE) -> Self {
        let lines = unsafe {
            let mut errorInfo = error_info();
            let mut rowCount: u32 = 0;
            let r = RfcGetRowCount(handle, &mut rowCount, &mut errorInfo);
            assert_eq!(0, r);
            rowCount
        };
        Self {
            handle,
            lines,
            tabix: 0,
        }
    }
}

impl Iterator for SapTableIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.tabix < self.lines {
            let x = unsafe {
                let mut errorInfo = error_info();
                let rc = RfcMoveTo(self.handle, self.tabix, &mut errorInfo);
                assert_eq!(0, rc);
                let struct_handle = RfcGetCurrentRow(self.handle, &mut errorInfo);
                self.tabix += 1;
                SapStructure {
                    handle: struct_handle,
                    dependent: true,
                }
            };
            Some(Value::Structure(x))
        } else {
            None
        }
    }
}

pub struct SapStructure {
    handle: RFC_STRUCTURE_HANDLE,
    dependent: bool,
}

impl SapStructure {
    pub fn get<S>(&self, name: S) -> Result<Value, String>
    where
        S: Into<String>,
    {
        let mut errorInfo = error_info();
        let mut fieldDescr = field_descriptor();
        let str_name: String = name.into();
        let sap_name = SapString::from(str_name);
        unsafe {
            let type_handle = RfcDescribeType(self.handle, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            let rc = RfcGetFieldDescByName(
                type_handle,
                sap_name.raw_pointer(),
                &mut fieldDescr,
                &mut errorInfo,
            );
            if rc != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_eq!(0, rc);
            match fieldDescr.type_ {
                _RFCTYPE_RFCTYPE_CHAR => {
                    let mut buffer = vec![0; fieldDescr.ucLength as usize + 1];
                    let rc = RfcGetChars(
                        self.handle,
                        fieldDescr.name.as_ptr(),
                        buffer.as_mut_ptr(),
                        fieldDescr.ucLength,
                        &mut errorInfo,
                    );
                    if errorInfo.code != 0 {
                        return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                    }
                    assert_eq!(0, rc);
                    Ok(Value::String(SapString::from(buffer.as_slice())))
                }
                _ => Ok(Value::Empty),
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        trace!("closing connection");
        if self.is_connected() {
            if let Ok(mut x) = CONNECT_COUNT.lock() {
                let mut errorInfo = error_info();
                unsafe {
                    RfcCloseConnection(self.cn, &mut errorInfo);
                }
                *x = *x - 1;
                trace!("close -> {} connections", *x);
            }
        }
    }
}

impl Drop for Function {
    fn drop(&mut self) {
        let mut errorInfo1 = error_info();
        let mut errorInfo2 = error_info();
        unsafe {
            RfcDestroyFunction(self.fh, &mut errorInfo2); // destry funtion handle first
            RfcDestroyFunctionDesc(self.fd, &mut errorInfo1);
        }
        trace!("drop function done");
    }
}
impl Drop for SapTable {
    #[tracing::instrument]
    fn drop(&mut self) {
        trace!("drop table");
        if !self.dependent {
            let mut errorInfo = error_info();
            unsafe {
                RfcDestroyTable(self.handle, &mut errorInfo);
            }
        }
        trace!("drop table");
    }
}
impl Drop for SapStructure {
    #[tracing::instrument]
    fn drop(&mut self) {
        trace!("drop structure");
        if !self.dependent {
            let mut errorInfo = error_info();
            unsafe {
                RfcDestroyStructure(self.handle, &mut errorInfo);
            }
        }
        trace!("drop structure done");
    }
}

#[derive(Debug)]
pub enum Value {
    Empty,
    String(SapString),
    Int(i64),
    Table(SapTable),
    Structure(SapStructure),
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
                    _RFCTYPE_RFCTYPE_CHAR => {
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

impl std::iter::IntoIterator for SapTable {
    type Item = Value;

    type IntoIter = SapTableIterator;

    fn into_iter(self) -> Self::IntoIter {
        SapTableIterator::new(self.handle)
    }
}

impl std::fmt::Debug for SapTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("SapTable");
        dbg.field("handle", &self.handle);
        unsafe {
            let mut err_info = error_info();
            let mut row_count: u32 = 0;
            let rc = RfcGetRowCount(self.handle, &mut row_count, &mut err_info);
            assert_eq!(0, rc);
            dbg.field("row-count", &Box::new(row_count) as &dyn std::fmt::Debug);
        }
        dbg.finish()
    }
}

impl Function {
    pub fn execute(&self) -> Result<(), String> {
        let mut errorInfo = error_info();
        unsafe {
            let rc = RfcInvoke(self.cn, self.fh as *mut RFC_DATA_CONTAINER, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_eq!(0, rc);
        }
        Ok(())
    }

    pub fn set<V>(&self, name: &str, value: V) -> Result<(), String>
    where
        V: Into<Value>,
    {
        let mut errorInfo = error_info();
        let str_name = SapString::from(name);
        if let Value::String(ss) = value.into() {
            unsafe {
                let rc = RfcSetChars(
                    self.fh,
                    str_name.raw_pointer(),
                    ss.raw_pointer(),
                    ss.len() as u32,
                    &mut errorInfo,
                );
                if rc != 0 {
                    let x = SapString::from(errorInfo.message.as_slice());
                    trace!("sap string created");
                    return Err(String::from(&x));
                }
                assert_eq!(0, rc);
            }
        }
        Ok(())
    }

    pub fn get(&self, _name: &str) -> Result<Value, String> {
        let mut paramDesc = parameter_description();
        let mut errorInfo = error_info();
        let name = SapString::from(_name);
        unsafe {
            RfcGetParameterDescByName(self.fd, name.raw_pointer(), &mut paramDesc, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
        }
        let v = match paramDesc.type_ {
            _RFCTYPE_RFCTYPE_INT => unsafe {
                let mut value: RFC_INT = 0;
                RfcGetInt(self.fh, name.raw_pointer(), &mut value, &mut errorInfo);
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                Value::Int(value as i64)
            },
            _RFCTYPE_RFCTYPE_STRUCTURE => unsafe {
                let mut structHandle: RFC_STRUCTURE_HANDLE = 0 as RFC_STRUCTURE_HANDLE;
                let rc = RfcGetStructure(
                    self.fh,
                    name.raw_pointer(),
                    &mut structHandle,
                    &mut errorInfo,
                );
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                trace!("structure handle: {:p}", structHandle);
                Value::Structure(SapStructure {
                    handle: structHandle,
                    dependent: true,
                })
            },
            _RFCTYPE_RFCTYPE_TABLE => unsafe {
                let mut handle: RFC_TABLE_HANDLE = 0 as RFC_TABLE_HANDLE;
                let rc = RfcGetTable(self.fh, name.raw_pointer(), &mut handle, &mut errorInfo);
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }
                assert_eq!(0, rc);
                trace!("table handle: {:p}", handle);
                Value::Table(SapTable {
                    handle,
                    dependent: true,
                })
            },
            _ => todo!(),
        };
        Ok(v)
    }
}

impl Connection {
    pub fn new() -> Self {
        Self {
            params: Vec::new(),
            cn: 0 as RFC_CONNECTION_HANDLE,
        }
    }
    pub fn is_connected(&self) -> bool {
        self.cn != (0 as RFC_CONNECTION_HANDLE)
    }

    pub fn destination(mut self, arg: &str) -> Self {
        self.params.push(RfcParam::new("dest", arg));
        self
    }

    pub fn get_params(&self) -> Vec<SapString> {
        let mut v = Vec::new();
        for x in self.params.iter() {
            v.push(x.name.clone());
        }
        v
    }

    pub fn connect(mut self) -> Result<Self, String> {
        let mut x = CONNECT_COUNT.lock().map_err(any_to_string)?;
        let ps = self
            .params
            .iter()
            .map(|x| _RFC_CONNECTION_PARAMETER {
                name: x.name.raw_pointer(),
                value: x.value.raw_pointer(),
            })
            .collect::<Vec<_RFC_CONNECTION_PARAMETER>>()
            .into_boxed_slice();
        let mut err_info = error_info();
        trace!("parameter count: {}", ps.len());
        let cn = unsafe { RfcOpenConnection(ps.as_ptr(), ps.len() as u32, &mut err_info) };
        trace!("cn: {cn:p}");
        trace!("par {:?}", ps[0]);
        dump_memory(self.params[0].name.raw_pointer());
        dump_memory(self.params[0].value.raw_pointer());
        if cn != 0 as *mut _RFC_CONNECTION_HANDLE {
            self.cn = cn;
            *x = *x + 1;
            trace!("open -> {} connections", *x);
        }
        trace!(
            "Key: {:}",
            String::from(&SapString::from(err_info.key.as_slice()))
        );
        trace!(
            "Message: {:}",
            String::from(&SapString::from(err_info.message.as_slice()))
        );
        if err_info.code != 0 {
            return Err(String::from(&SapString::from(err_info.message.as_slice())));
        }

        Ok(self)
    }

    pub fn function(&self, arg: &str) -> Result<Function, String> {
        let name = SapString::from(arg);
        let mut errorInfo = error_info();
        unsafe {
            let fd = RfcGetFunctionDesc(self.cn, name.raw_pointer(), &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert!(fd as usize != 0);

            let mut count: cty::c_uint = 0;
            let rc = RfcGetParameterCount(fd, &mut count as *mut u32, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_eq!(rc, 0);
            trace!("param count: {count}");
            for i in 0..count {
                let mut paramDesc = parameter_description();
                let rc = RfcGetParameterDescByIndex(fd, i, &mut paramDesc, &mut errorInfo);
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }

                assert_eq!(rc, 0);

                let s = paramDesc.name.as_slice();
                let n = String::from(&SapString::from(s));
                trace!("{i} {n} Type:{}", paramDesc.type_);
            }
            let fh = RfcCreateFunction(fd, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_ne!(0, fh as usize);
            Ok(Function {
                cn: self.cn,
                fd,
                fh,
            })
        }
    }
}

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

fn zero<const N: usize>() -> [u16; N] {
    [0; N]
}

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

fn dump_memory(name_ptr: *const u16) {
    trace!("name: {:p}", name_ptr);
    for i in 0..16 as usize {
        unsafe {
            trace!(" {:02x}", *(name_ptr.add(i)));
        }
    }
}
