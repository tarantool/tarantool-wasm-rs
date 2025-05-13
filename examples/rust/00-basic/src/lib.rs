#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::cli::run::Guest as CliGuest;
use bindings::exports::wasi::http::incoming_handler::Guest as HttpGuest;

struct Component;

impl CliGuest for Component {
    fn run() -> Result<(),()> {
        println!("Hello from WASI Component!");
        Ok(())
    }
}

impl HttpGuest for Component {
    fn handle(request: bindings::exports::wasi::http::incoming_handler::IncomingRequest,response_out: bindings::exports::wasi::http::incoming_handler::ResponseOutparam,) -> () {
        println!("Hello from WASI Component HTTP handler!");
    }
}

bindings::export!(Component with_types_in bindings);
