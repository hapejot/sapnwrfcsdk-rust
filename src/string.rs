use tracing::trace;

use crate::librfc::SAP_UC;

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

impl From<String> for SapString {
    fn from(value: String) -> Self {
        let mut v: Vec<u16> = Vec::new();
        ucs2::encode_with(value.as_str(), |c| Ok(v.push(c))).unwrap();
        v.push(0);
        SapString::new(v)
    }
}
