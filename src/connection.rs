use log::trace;

use crate::{
    any_to_string, error_info, function::Function, librfc::RfcCloseConnection,
    librfc::RfcCreateFunction, librfc::RfcGetFunctionDesc, librfc::RfcOpenConnection,
    librfc::RFC_CONNECTION_HANDLE, librfc::_RFC_CONNECTION_HANDLE,
    librfc::_RFC_CONNECTION_PARAMETER, rfc_param::RfcParam, string::SapString, CONNECT_COUNT,
};

pub struct Connection {
    cn: RFC_CONNECTION_HANDLE,
    params: Vec<RfcParam>,
}
impl Connection {
    /// Creates a new `Connection` instance with an empty parameter list and a connection handle set to zero.
    /// This function initializes the `Connection` struct with an empty vector for parameters
    /// and a connection handle set to zero, indicating that no connection has been established yet.
    pub fn new() -> Self {
        Self {
            params: Vec::new(),
            cn: 0 as RFC_CONNECTION_HANDLE,
        }
    }

    /// Checks if the connection is established by verifying if the connection handle is not zero.
    /// This function returns `true` if the connection handle is valid (not zero), indicating
    /// that the connection is established. Otherwise, it returns `false`.
    /// # Returns
    /// * `bool` - `true` if the connection is established, `false` otherwise.
    pub fn is_connected(&self) -> bool {
        self.cn != (0 as RFC_CONNECTION_HANDLE)
    }

    /// Adds a parameter to the connection with the specified name and value.
    /// This function creates a new `RfcParam` with the given name and value,
    /// and appends it to the connection's parameter list.
    /// # Arguments
    /// * `name` - A string slice representing the name of the parameter.
    /// * `value` - A string slice representing the value of the parameter.
    pub fn destination(mut self, arg: &str) -> Self {
        self.params.push(RfcParam::new("dest", arg));
        self
    }

    /// Returns a vector of parameter names for the connection.
    pub fn get_params(&self) -> Vec<SapString> {
        let mut v = Vec::new();
        for x in self.params.iter() {
            v.push(x.name().clone());
        }
        v
    }

    /// Connects to the SAP system using the provided parameters.
    /// This function attempts to open a connection to the SAP system using the parameters
    /// specified in the connection. It increments the connection count and returns a `Result`
    /// indicating success or failure.
    /// # Returns
    /// * `Result<Self, String>` - Returns `Ok(Self)` if the connection is successful,
    ///   or an `Err(String)` containing an error message if the connection fails.
    /// # Errors
    /// * Returns an error message if the connection fails, which can be caused by invalid parameters   
    pub fn connect(mut self) -> Result<Self, String> {
        let mut x = CONNECT_COUNT.lock().map_err(any_to_string)?;
        let ps = self
            .params
            .iter()
            .map(|x| _RFC_CONNECTION_PARAMETER {
                name: x.name().raw_pointer(),
                value: x.value().raw_pointer(),
            })
            .collect::<Vec<_RFC_CONNECTION_PARAMETER>>()
            .into_boxed_slice();
        let mut err_info = error_info();
        trace!("parameter count: {}", ps.len());
        let cn = unsafe { RfcOpenConnection(ps.as_ptr(), ps.len() as u32, &mut err_info) };
        trace!("cn: {cn:p}");
        trace!("par {:?}", ps[0]);
        // dump_memory(self.params[0].name.raw_pointer());
        // dump_memory(self.params[0].value.raw_pointer());
        if cn != 0 as *mut _RFC_CONNECTION_HANDLE {
            self.cn = cn;
            *x = *x + 1;
            trace!("open -> {} connections", *x);
        }
        trace!(
            "Key: {:}",
            String::from(&SapString::from(err_info.key.as_slice()))
        );
        trace!(
            "Message: {:}",
            String::from(&SapString::from(err_info.message.as_slice()))
        );
        if err_info.code != 0 {
            return Err(String::from(&SapString::from(err_info.message.as_slice())));
        }

        Ok(self)
    }

    /// Retrieves a function description for the specified function name.
    /// This function takes a string argument representing the function name,
    /// creates a `SapString` from it, and uses the SAP RFC API to get the function description.
    /// If the function description is successfully retrieved, it creates a new `Function` instance
    /// and returns it. If there is an error, it returns an `Error` with the error message.
    /// # Arguments
    /// * `arg` - A string slice representing the name of the function to retrieve.
    /// # Returns
    /// * `Result<Function, String>` - Returns `Ok(Function)` if the function is found,
    ///   or an `Err(String)` containing an error message if the function is not found.
    pub fn function(&self, arg: &str) -> Result<Function, String> {
        let name = SapString::from(arg);
        let mut errorInfo = error_info();
        unsafe {
            let fd = RfcGetFunctionDesc(self.cn, name.raw_pointer(), &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert!(fd as usize != 0);

            let fh = RfcCreateFunction(fd, &mut errorInfo);
            if errorInfo.code != 0 {
                return Err(String::from(&SapString::from(errorInfo.message.as_slice())));
            }
            assert_ne!(0, fh as usize);
            Function::new(self.cn, fh, fd)
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        trace!("closing connection");
        if self.is_connected() {
            if let Ok(mut x) = CONNECT_COUNT.lock() {
                let mut errorInfo = error_info();
                unsafe {
                    RfcCloseConnection(self.cn, &mut errorInfo);
                }
                *x = *x - 1;
                trace!("close -> {} connections", *x);
            }
        }
    }
}
