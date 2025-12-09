use clap::Parser;
use env_logger::Env;
use librfc_rust::connection::Connection;
use log::{info, trace};

#[derive(Debug, Parser)]
struct CommandLineArgs {
    pub rfc_destination: String,
    pub script_name: Option<String>,
}

fn main() -> Result<(), String> {
    // let args = CommandLineArgs {
    // rfc_destination: "sap".to_string(),
    // function_name: "RFC_SYSTEM_INFO".to_string(),

    // SDF/EWA_CHECK_OSMON           /SDF/EWA                                                      Function checks SAPWLSERV, OSMON, ...
    // SDF/EWA_GET_ABAP_DUMPS        /SDF/EWA                   S                                  EarlyWatch Alert: Get ABAP dumps
    // SDF/EWA_GET_CONFIG_FILES      /SDF/EWA_6X                                                   Creates table with file content of different files
    // SDF/EWA_GET_CPU_TIME          /SDF/EWA                   S                                  Get CPU time for standard operations
    // SDF/EWA_GET_DB_CONNECTIONS    /SDF/EWA                                                      Get DB connection details used by SAP instance
    // SDF/EWA_GET_ENQUE_STAT        /SDF/EWA                                                      Get Enqueue Statistics
    // SDF/EWA_GET_EWM_DATA          /SDF/EWA                                                      Data Collector for Extended Warehouse Management
    // SDF/EWA_GET_HARDWARE_INFO     /SDF/EWA                                                      Get Hardware Info from local and connected servers (incl. SAPOSCOL, XML)
    // SDF/EWA_GET_OSCOLL_INFO       /SDF/EWA                                                      Read Info from SAP OS Collector
    // SDF/EWA_GET_PARAMETER         /SDF/EWA                                                      Get a single value of a specific ABAP kernel parameter
    // SDF/EWA_GET_PARAMETER_DATA    /SDF/EWA                                                      Reading instance par. + PFLs (Default/Instance)
    // SDF/EWA_GET_PROCESSES         /SDF/EWA                                                      Get running processes on OS level
    // SDF/EWA_GET_UPDATE_ERRORS     /SDF/EWA                   S                                  EarlyWatch Alert: Get Update Errors
    // SDF/EWA_ICF_SERVICES          /SDF/EWA_6X                                                   Reads active ICF services from system
    // SDF/EWA_PERFORM_NORM_OP       /SDF/EWA                   S                                  Perform standard operations
    // SDF/EWA_READ_FILE_INTO_TABLE  /SDF/EWA_6X                                                   Reads a file in different ways
    // SDF/EWA_SAPOSCOL_B            /SDF/EWA                                                      Run saposcol -b
    // SDF/EWA_SAPWORKLOAD_CONFIG    /SDF/EWA_6X                                                   Get all customizing information from ST03(N) Workload Collector
    // SDF/EWA_STARTDAY              /SDF/EWA                                                      Returns Monday of the last complete week
    // script_name: "test-call.json".to_string(),
    // function_name: "/SDF/TEAP_TMS_GET_HISTORY".to_string(),
    // parameters: String::from(r#"{"IV_SYSTEM": "KGQ", "IV_IMPORTS": "X"}"#),
    // };
    let args = CommandLineArgs::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let script: serde_json::Value = if let Some(script_name) = &args.script_name {
        serde_yaml::from_reader(
            std::fs::File::open(script_name)
                .map_err(|e| format!("Failed to open script file: {}", e))?,
        )
    } else {
        serde_yaml::from_reader(std::io::stdin())
    }
    .map_err(|e| e.to_string())?;

    if let serde_json::Value::Array(steps) = script {
        info!("connecting");
        let c = Connection::new()
            .destination(&args.rfc_destination)
            .connect()?;

        assert!(c.is_connected());

        for step in steps {
            let function_name = step
                .get("function_name")
                .and_then(|f| f.as_str())
                .ok_or("Function name not found in step".to_string())?;
            info!("calling function {function_name}");
            let f = c.function(function_name)?;
            if let Some(p) = step.get("parameters") {
                f.set_parameters(&p)?;
            }
            f.execute()?;
            trace!("serializing result");
            serde_json::to_writer(std::io::stdout(), &f).map_err(|x| x.to_string())?;
            trace!("serializing result done");
        }
    }
    Ok(())
}
