use log::trace;

use crate::{
    check_ptr, check_rc,
    error::RfcError,
    error_info,
    function::Function,
    librfc::{
        RfcCloseConnection, RfcCreateFunction, RfcGetFunctionDesc, RfcOpenConnection,
        RfcSetIniPath, RFC_CONNECTION_HANDLE, _RFC_CONNECTION_PARAMETER,
    },
    lock,
    rfc_param::RfcParam,
    string::SapString,
    RfcResult, CONNECT_COUNT, INI_PATH_INITIALIZED,
};

pub struct Connection {
    cn: RFC_CONNECTION_HANDLE,
    params: Vec<RfcParam>,
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection {
    /// Creates a new `Connection` with no parameters and no open connection handle.
    pub fn new() -> Self {
        Self {
            params: Vec::new(),
            cn: std::ptr::null_mut(),
        }
    }

    /// Returns `true` if a connection handle is held (i.e. `connect` succeeded).
    pub fn is_connected(&self) -> bool {
        !self.cn.is_null()
    }

    /// Setzt das SAP-Ziel (`dest`) aus der sapnwrfc.ini bzw. der RFC_INI-Konfiguration.
    pub fn destination(mut self, arg: &str) -> Self {
        self.params.push(RfcParam::new("dest", arg));
        self
    }

    /// Returns a vector of parameter names for the connection.
    pub fn get_params(&self) -> Vec<SapString> {
        self.params.iter().map(|x| x.name().clone()).collect()
    }

    /// Opens the connection to the SAP system using the configured parameters.
    /// # Errors
    /// * `InvalidInput`, wenn keine Parameter gesetzt wurden.
    /// * `Sdk`, wenn der INI-Pfad nicht gesetzt oder der Logon abgelehnt wird.
    pub fn connect(mut self) -> RfcResult<Self> {
        // RFC_INI-Pfad nur einmal setzen, wenn die Umgebungsvariable gesetzt ist.
        {
            let mut ini_initialized = lock(&INI_PATH_INITIALIZED)?;
            if !*ini_initialized {
                if let Ok(rfc_ini_path) = std::env::var("RFC_INI") {
                    let ini_path = SapString::from(rfc_ini_path);
                    let mut ei = error_info();
                    let rc = unsafe { RfcSetIniPath(ini_path.raw_pointer(), &mut ei) };
                    check_rc(rc, &ei)?;
                    trace!("RFC INI path set to: {:?}", ini_path);
                }
                *ini_initialized = true;
            }
        }

        if self.params.is_empty() {
            return Err(RfcError::InvalidInput(
                "keine Verbindungsparameter gesetzt (z. B. destination(...) aufrufen)".to_string(),
            ));
        }

        let mut count = lock(&CONNECT_COUNT)?;
        let ps = self
            .params
            .iter()
            .map(|x| _RFC_CONNECTION_PARAMETER {
                name: x.name().raw_pointer(),
                value: x.value().raw_pointer(),
            })
            .collect::<Vec<_RFC_CONNECTION_PARAMETER>>()
            .into_boxed_slice();
        trace!("parameter count: {}", ps.len());

        let mut ei = error_info();
        let cn = unsafe { RfcOpenConnection(ps.as_ptr(), ps.len() as u32, &mut ei) };
        let cn = check_ptr(cn, &ei, "RfcOpenConnection")?;
        self.cn = cn;
        *count += 1;
        trace!("open -> {} connections", *count);
        Ok(self)
    }

    /// Retrieves a function description for the specified function name and creates a call handle.
    /// # Errors
    /// * `Sdk`, wenn die Funktion nicht existiert oder nicht angelegt werden kann.
    pub fn function(&self, arg: &str) -> RfcResult<Function> {
        let name = SapString::from(arg);

        let mut ei = error_info();
        let fd = unsafe { RfcGetFunctionDesc(self.cn, name.raw_pointer(), &mut ei) };
        let fd = check_ptr(fd, &ei, "RfcGetFunctionDesc")?;

        let mut ei = error_info();
        let fh = unsafe { RfcCreateFunction(fd, &mut ei) };
        let fh = check_ptr(fh, &ei, "RfcCreateFunction")?;

        Function::new(self.cn, fh, fd)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if self.is_connected() {
            trace!("closing connection");
            if let Ok(mut count) = lock(&CONNECT_COUNT) {
                let mut ei = error_info();
                unsafe {
                    RfcCloseConnection(self.cn, &mut ei);
                }
                *count -= 1;
                trace!("close -> {} connections", *count);
            }
        }
    }
}
