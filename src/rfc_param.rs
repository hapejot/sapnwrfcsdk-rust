use crate::string::SapString;

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

pub fn name(&self) -> &SapString {
    &self.name
}

pub fn value(&self) -> &SapString {
    &self.value
}
}
