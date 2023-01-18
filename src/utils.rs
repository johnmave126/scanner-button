use std::fmt::Display;

use log::error;

pub const BJNP_PORT: u16 = 8612;

pub fn ignore_err<T, E: Display>(x: Result<T, E>) -> Option<T> {
    match x {
        Ok(t) => Some(t),
        Err(e) => {
            error!("{e}");
            None
        }
    }
}
