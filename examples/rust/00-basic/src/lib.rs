#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::cli::run::Guest as CliGuest;
use bindings::exports::wasi::http::incoming_handler::Guest as HttpGuest;

struct Component;

impl CliGuest for Component {
    fn run() -> Result<(),()> {
        println!("Hello from Rust WASM!");
        Ok(())
    }
}

impl HttpGuest for Component {
    fn handle(_request: bindings::exports::wasi::http::incoming_handler::IncomingRequest, _response_out: bindings::exports::wasi::http::incoming_handler::ResponseOutparam,) -> () {}
}

bindings::export!(Component with_types_in bindings);
