use std::io::{stdout, Write};

use librfc_rust::{Connection, Value};
use env_logger;
use env_logger::Env;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    id: String,
    #[clap(short, long, default_value = "DE")]
    language: String,
    object: String,
}


fn main() {
    let args = Args::parse();
    env_logger::Builder::from_env(Env::default()
                        .default_filter_or("info"))
                    .format_timestamp(None)
                    .init();
    let c = Connection::new()
                .destination("sap")
                .connect();
    assert!(c.is_connected());
    let f = c.function("DOCU_GET");
    f.set("ID", args.id.as_str());
    f.set("LANGU", args.language.as_str());
    f.set("OBJECT", args.object.as_str());
    f.execute();
    if let Value::Table(functions) = f.get("LINE") {
        for row in functions {
            if let Value::Structure(s) = row {
                let format = format!("{:2}", s.get("TDFORMAT").to_string());
                let line = format!("{:}", s.get("TDLINE").to_string());
                stdout().write(format.as_bytes()).unwrap();
                stdout().write(line.as_bytes()).unwrap();
                stdout().write(b"\n").unwrap();
            }
        }
    }
}
