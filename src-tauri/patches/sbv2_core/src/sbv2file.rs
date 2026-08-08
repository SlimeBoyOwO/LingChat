use std::io::{Cursor, Read};

use tar::Archive;
use zstd::decode_all;

use crate::error::{Error, Result};

/// Parse a .sbv2 file binary
///
/// # Examples
///
/// ```rs
/// parse_sbv2file("tsukuyomi", std::fs::read("tsukuyomi.sbv2")?)?;
/// ```
pub fn parse_sbv2file<P: AsRef<[u8]>>(sbv2_bytes: P) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut arc = Archive::new(Cursor::new(decode_all(Cursor::new(sbv2_bytes.as_ref()))?));
    let mut vits2 = None;
    let mut style_vectors = None;
    let mut et = arc.entries()?;
    while let Some(Ok(mut e)) = et.next() {
        let pth = String::from_utf8_lossy(&e.path_bytes()).to_string();
        let mut b = Vec::with_capacity(e.size() as usize);
        e.read_to_end(&mut b)?;
        match pth.as_str() {
            "model.onnx" => vits2 = Some(b),
            "style_vectors.json" => style_vectors = Some(b),
            _ => continue,
        }
    }
    if style_vectors.is_none() {
        return Err(Error::ModelNotFoundError("style_vectors".to_string()));
    }
    if vits2.is_none() {
        return Err(Error::ModelNotFoundError("vits2".to_string()));
    }
    Ok((style_vectors.unwrap(), vits2.unwrap()))
}
