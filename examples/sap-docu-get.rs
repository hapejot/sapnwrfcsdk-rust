use std::io::{stdout, Write};

use librfc_rust::{Connection, Value};
use env_logger::Env;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    id: String,
    #[clap(short, long, default_value = "DE")]
    language: String,
    object: String,
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    env_logger::Builder::from_env(Env::default()
                        .default_filter_or("info"))
                    .format_timestamp(None)
                    .init();
    let c = Connection::new()
                .destination("sap")
                .connect()?;
    assert!(c.is_connected());
    let f = c.function("DOCU_GET")?;
    f.set("ID", args.id.to_uppercase().as_str())?;
    f.set("LANGU", args.language.to_uppercase().as_str())?;
    f.set("OBJECT", args.object.to_uppercase().as_str())?;
    f.execute()?;
    let mut out = stdout().lock();
    if let Value::Table(functions) = f.get("LINE")? {
        for row in functions.into_iter() {
            if let Value::Structure(s) = row {
                let format = format!("{:2}", s.get("TDFORMAT")?.to_string());
                let line = s.get("TDLINE")?.to_string();
                out.write_all(format.as_bytes()).map_err(|e| e.to_string())?;
                out.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
                out.write_all(b"\n").map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
