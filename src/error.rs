use std::fmt;

/// Ergebnistyp aller Funktionen dieser Bibliothek.
pub type RfcResult<T> = Result<T, RfcError>;

/// ABAP-Meldung, die SAP zusammen mit einem RFC-Fehler liefern kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbapMessage {
    pub class: String,
    pub msg_type: String,
    pub number: String,
    pub v1: String,
    pub v2: String,
    pub v3: String,
    pub v4: String,
}

/// Vom SAP-RFC-SDK gemeldeter Fehler (Inhalt von `RFC_ERROR_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkError {
    pub code: i32,
    pub group: i32,
    pub key: String,
    pub message: String,
    pub abap: Option<AbapMessage>,
}

/// Fehler der RFC-Bibliothek.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfcError {
    /// Vom SAP-RFC-SDK gemeldeter Fehler. Boxed, damit `RfcError` klein bleibt.
    Sdk(Box<SdkError>),
    /// Das SDK hat einen Null-Handle geliefert, ohne einen Fehler zu melden.
    NullHandle { operation: &'static str },
    /// Der Feldtyp wird von dieser Bibliothek nicht unterstützt.
    UnsupportedType { name: String, type_code: i32 },
    /// Ein JSON-Schlüssel passt zu keinem Feld bzw. Parameter der Struktur.
    UnknownField { name: String },
    /// Ein Funktionsparameter existiert nicht.
    UnknownParameter { name: String },
    /// Der JSON-Wert passt nicht zum Typ des Feldes.
    TypeMismatch {
        name: String,
        expected: &'static str,
        found: &'static str,
    },
    /// Die Eingabe ist strukturell ungültig (z. B. fehlendes Ziel, falscher Wurzeltyp).
    InvalidInput(String),
    /// Interner Zustand ist ungültig (z. B. vergiftete Mutex).
    Internal(String),
}

impl fmt::Display for RfcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RfcError::Sdk(e) => {
                if e.message.trim().is_empty() {
                    write!(f, "RFC-Fehler (Code {})", e.code)?;
                } else {
                    write!(f, "{}", e.message)?;
                }
                if let Some(a) = &e.abap {
                    write!(f, " [ABAP {} {} {}]", a.class, a.msg_type, a.number)?;
                }
                Ok(())
            }
            RfcError::NullHandle { operation } => {
                write!(f, "{operation} lieferte kein Handle")
            }
            RfcError::UnsupportedType { name, type_code } => {
                write!(f, "Nicht unterstützter SAP-Typ {type_code} bei {name}")
            }
            RfcError::UnknownField { name } => {
                write!(f, "Unbekanntes Feld oder Schlüssel: {name}")
            }
            RfcError::UnknownParameter { name } => {
                write!(f, "Unbekannter Parameter: {name}")
            }
            RfcError::TypeMismatch {
                name,
                expected,
                found,
            } => write!(f, "{name}: erwartet {expected}, gefunden {found}"),
            RfcError::InvalidInput(msg) => write!(f, "Ungültige Eingabe: {msg}"),
            RfcError::Internal(msg) => write!(f, "Interner Fehler: {msg}"),
        }
    }
}

impl std::error::Error for RfcError {}
