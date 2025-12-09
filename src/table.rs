use log::trace;
use serde::{ser::SerializeSeq, Serialize};

use crate::{
    error_info,
    librfc::{
        RfcAppendRow, RfcDescribeType, RfcDestroyTable, RfcGetCurrentRow, RfcGetRowCount,
        RfcMoveTo, RFC_TABLE_HANDLE,
    },
    string::SapString,
    structure::SapStructure,
    value::Value,
};

pub struct SapTable {
    handle: RFC_TABLE_HANDLE,
    dependent: bool,
}

impl SapTable {
    pub fn new(handle: RFC_TABLE_HANDLE, dependent: bool) -> Self {
        Self { handle, dependent }
    }
    pub fn len(&self) -> usize {
        unsafe {
            let mut errorInfo = error_info();
            let mut rowCount: u32 = 0;
            let rc = RfcGetRowCount(self.handle, &mut rowCount, &mut errorInfo);
            assert_eq!(0, rc);
            rowCount as usize
        }
    }

    pub fn handle(&self) -> RFC_TABLE_HANDLE {
        self.handle
    }

    pub fn add_row(&self, row: &serde_json::Value) -> Result<(), String> {
        if let serde_json::Value::Object(obj) = row {
            unsafe {
                let mut errorInfo = error_info();
                let type_handle = RfcDescribeType(self.handle, &mut errorInfo);
                if errorInfo.code != 0 {
                    return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
                }

                let structure = SapStructure::from_type(type_handle);
                for (name, value) in obj {
                    match value {
                        serde_json::Value::Null => todo!(),
                        serde_json::Value::Bool(_v) => todo!(),
                        serde_json::Value::Number(_number) => todo!(),
                        serde_json::Value::String(v) => structure.set(name, v.as_str())?,
                        serde_json::Value::Array(_values) => todo!(),
                        serde_json::Value::Object(_map) => todo!(),
                    }
                }
                RfcAppendRow(self.handle, structure.handle(), &mut errorInfo);
            }
        }
        Ok(())
    }
}

impl Serialize for SapTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = self.len();
        trace!("Serializing SapTable with {} rows", len);
        let mut tab = serializer.serialize_seq(Some(len))?;
        for value in self.into_iter() {
            tab.serialize_element(&value)?;
        }
        trace!("Serialized SapTable with {} rows", len);
        tab.end()
    }
}

pub struct SapTableIterator {
    handle: RFC_TABLE_HANDLE,
    lines: u32,
    tabix: u32,
}

impl std::iter::IntoIterator for &SapTable {
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
                SapStructure::new(struct_handle, true).unwrap()
            };
            Some(Value::Structure(x))
        } else {
            None
        }
    }
}

impl Drop for SapTable {
    fn drop(&mut self) {
        if !self.dependent {
            let mut errorInfo = error_info();
            unsafe {
                RfcDestroyTable(self.handle, &mut errorInfo);
            }
        }
    }
}
