pub mod discover;
mod header;
pub mod identity;
pub mod packet;
pub mod poll;
pub mod serdes;

const DISPLAY_INDENT: usize = 4;
macro_rules! write_nested {
    ($f: expr, $obj: expr) => {{
        if $f.sign_minus() {
            $f.write_fmt(format_args!(" / {obj:-}", obj = $obj))
        } else {
            let indent = $f.width().unwrap_or(0);
            let step = $f.precision().unwrap_or(crate::DISPLAY_INDENT);
            $f.write_fmt(format_args!(
                "\n{obj:width$.precision$}",
                obj = $obj,
                width = indent + step,
                precision = step
            ))
        }
    }};
}
pub(crate) use write_nested;

pub use crate::{packet::*, poll::command::Host};
