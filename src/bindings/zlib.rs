use crate::bindings::docs::adder::zlib;
use crate::tarantool_wasm::SyncWasmStates;

use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use std::io::Write;

impl zlib::Host for SyncWasmStates {
    async fn compress(&mut self, data: Vec<u8>, level: u32) -> Vec<u8> {
        let compression = Compression::new(level.min(9));
        let mut encoder = ZlibEncoder::new(Vec::new(), compression);
        encoder.write_all(&data).unwrap();
        encoder.finish().unwrap()
    }

    async fn decompress(&mut self, data: Vec<u8>) -> Vec<u8> {
        let mut decoder = ZlibDecoder::new(Vec::new());
        decoder.write_all(&data).unwrap();
        decoder.finish().unwrap()
    }

    async fn crc32(&mut self, data: Vec<u8>) -> u32 {
        crc32fast::hash(&data)
    }

    async fn adler32(&mut self, data: Vec<u8>) -> u32 {
        adler::adler32_slice(&data)
    }
}
