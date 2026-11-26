use log::trace;

use crate::librfc::SAP_UC;

/// UTF-16-Zeichenfolge für die SAP-API. Der Puffer endet immer mit einem Null-Terminator.
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

    /// Länge ohne Null-Terminator, in UTF-16-Einheiten.
    pub fn len(&self) -> usize {
        self.vec.len().saturating_sub(1)
    }
}

impl std::fmt::Debug for SapString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SapString({:?})", String::from(self))
    }
}

impl From<&str> for SapString {
    /// Kodiert nach UTF-16. Zeichen außerhalb der BMP werden als Surrogate-Paar kodiert,
    /// statt wie zuvor zu einem Panic zu führen.
    fn from(value: &str) -> Self {
        let mut v: Vec<u16> = value.encode_utf16().collect();
        v.push(0);
        SapString::new(v)
    }
}

impl From<&SapString> for String {
    /// Dekodiert nach UTF-8. Nachgestellte Leerzeichen und Null-Zeichen werden entfernt,
    /// ungültige Surrogate ersetzt (statt zu panicen).
    fn from(value: &SapString) -> Self {
        let end = value
            .vec
            .iter()
            .rposition(|&c| c != 0 && c != 32)
            .map_or(0, |i| i + 1);
        trace!("ucs2 vector created with {} elements", end);
        String::from_utf16_lossy(&value.vec[..end])
    }
}

impl From<&[u16]> for SapString {
    fn from(value: &[u16]) -> Self {
        trace!("from u16 slice");
        let mut v: Vec<u16> = value.iter().copied().filter(|&c| c > 0).collect();
        v.push(0);
        trace!("end of string found {}", v.len());
        SapString::new(v)
    }
}

impl From<String> for SapString {
    fn from(value: String) -> Self {
        SapString::from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::SapString;

    #[test]
    fn roundtrip_trims_trailing_blanks() {
        let s = SapString::from("ABC   ");
        assert_eq!(String::from(&s), "ABC");
        assert_eq!(s.len(), 6);
    }

    #[test]
    fn non_bmp_does_not_panic() {
        let s = SapString::from("a😀b");
        assert_eq!(String::from(&s), "a😀b");
    }

    #[test]
    fn invalid_surrogate_is_replaced_not_panicking() {
        let raw: [u16; 2] = [0xD800, 0x0041];
        let s = SapString::from(&raw[..]);
        let decoded = String::from(&s);
        assert!(decoded.contains('\u{FFFD}'));
        assert!(decoded.ends_with('A'));
    }
}
