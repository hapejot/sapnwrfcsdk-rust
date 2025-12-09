use env_logger;
use env_logger::Env;
use librfc_rust::{value::Value, connection::Connection};
use log::info;

fn main() -> Result<(), String> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();
    info!("rfc connect");
    let c = Connection::new().destination("sap").connect()?;
    assert!(c.is_connected());
    let f = c.function("DOCU_GET")?;
    f.set("ID", "SD")?;
    f.set("LANGU", "DE")?;
    f.set("OBJECT", "ABAPCOMPUTE_STRING_FORMAT_OPTIONS")?;
    f.execute()?;
    info!("{:?}", f.get("LINE"));
    if let Value::Table(functions) = f.get("LINE")? {
        for row in functions.into_iter() {
            if let Value::Structure(s) = row {
                info!("{:2} {}", s.get("TDFORMAT")?, s.get("TDLINE")?);
            }
        }
    }
    info!("done.");
    Ok(())
}
