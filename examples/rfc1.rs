use log::{info, trace, warn};
use librfc_rust::{Connection, Value};
use env_logger;
use env_logger::Env;


fn main() {
    env_logger::Builder::from_env(Env::default()
                        .default_filter_or("info"))
                    .format_timestamp(None)
                    .init();
    info!("rfc connect");
    let c = Connection::new()
                .destination("sap")
                .connect();
    assert!(c.is_connected());
    let f = c.function("RFC_FUNCTION_SEARCH");
    f.set("FUNCNAME", "Z*");
    f.execute();
    info!("{:?}", f.get("FUNCTIONS"));
    if let Value::Table(functions) = f.get("FUNCTIONS") {
        for row in functions {
            if let Value::Structure(s) = row {
                info!("{:?}", s.get("FUNCNAME"));
            }
        }
    }
    info!("done.");
}
