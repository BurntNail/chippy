#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

pub mod events;
pub mod game_types;
pub mod integer;
pub mod ser_glue;

#[must_use]
pub fn display_bytes_as_hex_array(b: &[u8]) -> String {
    use std::fmt::Write;

    let mut out;
    match b.len() {
        0 => out = "[]".to_string(),
        1 => out = format!("[{:#X}]", b[0]),
        _ => {
            out = format!("[{:#X}", b[0]);
            for b in b.iter().skip(1) {
                let _ = write!(&mut out, ", {b:#X}");
            }
            out.push(']');
        }
    }
    out
}
