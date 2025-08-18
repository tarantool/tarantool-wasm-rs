use crate::bindings::docs::adder::zlib;
use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use std::io::Write;
use tarantool::error::TarantoolErrorCode;

impl zlib::Host for SyncWasmStates {
    async fn compress(&mut self, data: Vec<u8>, level: u32) -> Vec<u8> {
        let compression = Compression::new(level.min(9));
        let mut encoder = ZlibEncoder::new(Vec::new(), compression);

        if let Err(e) = encoder.write_all(&data) {
            let err = BoxError::new(TarantoolErrorCode::Unknown as u32, e.to_string())
                .with_file(file!())
                .with_line(line!());

            println!("{err}");
            return Vec::new();
        }

        match encoder.finish() {
            Ok(res) => res,
            Err(e) => {
                let err = BoxError::new(TarantoolErrorCode::Unknown as u32, e.to_string())
                    .with_file(file!())
                    .with_line(line!());

                println!("{err}");
                Vec::new()
            }
        }
    }

    async fn decompress(&mut self, data: Vec<u8>) -> Vec<u8> {
        let mut decoder = ZlibDecoder::new(Vec::new());

        if let Err(e) = decoder.write_all(&data) {
            let err = BoxError::new(TarantoolErrorCode::Unknown as u32, e.to_string())
                .with_file(file!())
                .with_line(line!());

            println!("{err}");
            return Vec::new();
        }

        match decoder.finish() {
            Ok(res) => res,
            Err(e) => {
                let err = BoxError::new(TarantoolErrorCode::Unknown as u32, e.to_string())
                    .with_file(file!())
                    .with_line(line!());

                println!("{err}");
                Vec::new()
            }
        }
    }

    async fn crc32(&mut self, data: Vec<u8>) -> u32 {
        crc32fast::hash(&data)
    }

    async fn adler32(&mut self, data: Vec<u8>) -> u32 {
        adler::adler32_slice(&data)
    }
}
