use librfc_rust::{Connection, Value};

struct Sample {
    x: usize,
    y: i64,
}

impl std::fmt::Debug for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sample")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

#[test]
fn new_connection_object() {
    let c = Connection::new();
    assert!(!c.is_connected());
}

#[test]
fn connect_dest() {
    let mut c = Connection::new();
    c = c.destination("sap");
    c = c.connect().unwrap();
    assert!(c.is_connected());
}

#[test]
fn connect_fluent() {
    let c = Connection::new().destination("sap").connect().unwrap();
    assert!(c.is_connected());
}

#[test]
fn ping() {
    let c = Connection::new().destination("sap").connect().unwrap();
    assert!(c.is_connected());
    let f = c.function("RFC_SYSTEM_INFO").unwrap();
    f.execute().unwrap();
    println!("Max Resources: {:?}", f.get("MAXIMAL_RESOURCES"));
    println!("Struct: {:#?}", f.get("RFCSI_EXPORT"));
}

#[test]
fn search_function() {
    let c = Connection::new().destination("sap").connect().unwrap();
    assert!(c.is_connected());
    let f = c.function("RFC_FUNCTION_SEARCH").unwrap();
    f.set("FUNCNAME", "Z*").unwrap();
    f.execute().unwrap();
    println!("{:#?}", f.get("FUNCTIONS"));
    if let Value::Table(functions) = f.get("FUNCTIONS").unwrap() {
        for row in functions {
            if let Value::Structure(s) = row {
                println!("{:#?}", s.get("FUNCNAME"));
            }
        }
    }
}
