use log::{error, trace};
use serde::{ser::SerializeSeq, Serialize};

use crate::{
    check_ptr, check_rc,
    error::RfcError,
    error_info, json_kind,
    librfc::{
        RfcAppendRow, RfcDescribeType, RfcDestroyTable, RfcGetCurrentRow, RfcGetRowCount,
        RfcMoveTo, RFC_TABLE_HANDLE,
    },
    structure::SapStructure,
    value::Value,
    RfcResult,
};

pub struct SapTable {
    handle: RFC_TABLE_HANDLE,
    dependent: bool,
}

/// Liest die Zeile `index` (0-basiert) einer Tabelle.
fn read_row(handle: RFC_TABLE_HANDLE, index: u32) -> RfcResult<SapStructure> {
    let mut ei = error_info();
    let rc = unsafe { RfcMoveTo(handle, index, &mut ei) };
    check_rc(rc, &ei)?;

    let mut ei = error_info();
    let struct_handle = unsafe { RfcGetCurrentRow(handle, &mut ei) };
    let struct_handle = check_ptr(struct_handle, &ei, "RfcGetCurrentRow")?;
    SapStructure::new(struct_handle, true)
}

impl SapTable {
    pub fn new(handle: RFC_TABLE_HANDLE, dependent: bool) -> Self {
        Self { handle, dependent }
    }

    /// Anzahl der Zeilen.
    pub fn len(&self) -> RfcResult<usize> {
        let mut ei = error_info();
        let mut row_count: u32 = 0;
        let rc = unsafe { RfcGetRowCount(self.handle, &mut row_count, &mut ei) };
        check_rc(rc, &ei)?;
        Ok(row_count as usize)
    }

    pub fn handle(&self) -> RFC_TABLE_HANDLE {
        self.handle
    }

    /// Fügt eine Zeile aus einem JSON-Objekt hinzu. Alle Werte müssen Strings sein.
    pub fn add_row(&self, row: &serde_json::Value) -> RfcResult<()> {
        let obj = match row {
            serde_json::Value::Object(obj) => obj,
            other => {
                return Err(RfcError::TypeMismatch {
                    name: "table row".to_string(),
                    expected: "object",
                    found: json_kind(other),
                })
            }
        };

        let mut ei = error_info();
        let type_handle = unsafe { RfcDescribeType(self.handle, &mut ei) };
        let type_handle = check_ptr(type_handle, &ei, "RfcDescribeType")?;

        let structure = SapStructure::from_type(type_handle)?;
        for (name, value) in obj {
            match value {
                serde_json::Value::String(v) => structure.set(name, v.as_str())?,
                other => {
                    return Err(RfcError::TypeMismatch {
                        name: name.clone(),
                        expected: "string",
                        found: json_kind(other),
                    })
                }
            }
        }
        let mut ei = error_info();
        let rc = unsafe { RfcAppendRow(self.handle, structure.handle(), &mut ei) };
        check_rc(rc, &ei)
    }
}

impl Serialize for SapTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = self.len().map_err(serde::ser::Error::custom)?;
        trace!("Serializing SapTable with {} rows", len);
        let mut seq = serializer.serialize_seq(Some(len))?;
        for index in 0..len {
            let row = read_row(self.handle, index as u32).map_err(serde::ser::Error::custom)?;
            seq.serialize_element(&Value::Structure(row))?;
        }
        trace!("Serialized SapTable with {} rows", len);
        seq.end()
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
        match self.len() {
            Ok(row_count) => dbg.field("row-count", &row_count),
            Err(e) => dbg.field("row-count", &format!("<error: {e}>")),
        };
        dbg.finish()
    }
}

impl SapTableIterator {
    pub fn new(handle: RFC_TABLE_HANDLE) -> Self {
        // `IntoIterator` kann keinen Fehler melden. Schlägt die Zeilenzahl fehl, wird das
        // geloggt und die Iteration bleibt leer. Wer Fehler sehen will, nutzt `len()`/Serialize.
        let lines = match SapTable::new(handle, true).len() {
            Ok(n) => n as u32,
            Err(e) => {
                error!("Zeilenzahl der Tabelle nicht lesbar: {e}");
                0
            }
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
        if self.tabix >= self.lines {
            return None;
        }
        let index = self.tabix;
        self.tabix += 1;
        match read_row(self.handle, index) {
            Ok(row) => Some(Value::Structure(row)),
            Err(e) => {
                error!("Zeile {index} der Tabelle nicht lesbar: {e}");
                None
            }
        }
    }
}

impl Drop for SapTable {
    fn drop(&mut self) {
        if !self.dependent {
            let mut ei = error_info();
            unsafe {
                RfcDestroyTable(self.handle, &mut ei);
            }
        }
    }
}
