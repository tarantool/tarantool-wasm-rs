#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::cli::run::Guest as CliGuest;
use bindings::exports::wasi::http::incoming_handler::{Guest as HttpGuest, IncomingRequest, ResponseOutparam};

use bindings::tarantool::tarantool::{ttbox, msgpack, say, box_tuple};

struct Component;

impl HttpGuest for Component {
    fn handle(_req: IncomingRequest, _res: ResponseOutparam) {
        say::say_info("Handling incoming HTTP request from Rust", None);
    }
}

impl CliGuest for Component {
    fn run() -> Result<(), ()> {
        // Schema version
        let version = ttbox::schema_version();
        say::say_info(&format!("RUST | Schema version: {}", version), None);

        // Get space by name
        let space_name = "test_space".to_string();
        let space = match ttbox::space_by_name(&space_name) {
            Ok(s) => {
                say::say_info(&format!("RUST | Space found: {:?}", s), None);
                s
            }
            Err(e) => {
                say::say_error(&format!("RUST | Error: couldn't find space '{}': {:?}", space_name, e), None);
                return Err(());
            }
        };

        // Get index by name
        let index_name = "primary".to_string();
        let index = match ttbox::index_by_name(space, &index_name) {
            Ok(i) => {
                say::say_info(&format!("RUST | Index found: {:?}", i), None);
                i
            }
            Err(e) => {
                say::say_error(&format!("RUST | Error: couldn't find index '{}': {:?}", index_name, e), None);
                return Err(());
            }
        };

        // Encode tuple [1, "bar"]
        let tuple_value = serde_json::json!([1, "bar"]);
        let encoded_tuple = match msgpack::encode(&tuple_value.to_string().as_bytes().to_vec()) {
            Ok(buf) => buf,
            Err(e) => {
                say::say_error(&format!("RUST | Encoding error: {:?}", e), None);
                return Err(());
            }
        };

        // Insert tuple
        let insert_result = match ttbox::insert(space, &encoded_tuple) {
            Ok(tup_ptr) => {
                say::say_info(&format!("RUST | Insert successful, ptr = {:?}", tup_ptr), None);
                tup_ptr
            }
            Err(e) => {
                say::say_error(&format!("RUST | Insert failed: {:?}", e), None);
                return Err(());
            }
        };

        // Decode inserted tuple
        let tuple_buf = box_tuple::to_buf(insert_result);
        match tuple_buf {
            Ok(buf) => {
                match msgpack::decode(&buf) {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        say::say_info(&format!("RUST | Decoded inserted tuple: {}", text), None);
                    }
                    Err(e) => {
                        say::say_error(&format!("RUST | Decode error: {:?}", e), None);
                    }
                }
            }
            Err(e) => {
                say::say_error(&format!("RUST | to_buf failed: {:?}", e), None);
            }
        }

        // Prepare key and ops for update
        let key = msgpack::encode(&serde_json::json!([1]).to_string().as_bytes().to_vec()).unwrap();
        let ops = msgpack::encode(&serde_json::json!([["=", 2, "new_value"]]).to_string().as_bytes().to_vec()).unwrap();

        // Update tuple
        match ttbox::update(index, &key, &ops) {
            Ok(updated_ptr) => {
                say::say_info(&format!("RUST | Update successful, tuple_ptr = {:?}", updated_ptr), None);
            }
            Err(e) => {
                say::say_error(&format!("RUST | Update failed: {:?}", e), None);
                return Err(());
            }
        }

        Ok(())
    }
}

bindings::export!(Component with_types_in bindings);
