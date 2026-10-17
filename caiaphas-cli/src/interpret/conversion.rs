//! Conversions read a rule's text as a Rust type. The interpreter only has the
//! types of the standard library, so a rule that converts to a type of the
//! program's own takes any text its grammar matches.

use caiaphas::__private::{Parser, Rule, Width};

/// The paths of the standard library a conversion can name a type by.
const PATHS: &[&str] = &[
    "std::string::",
    "alloc::string::",
    "std::primitive::",
    "core::primitive::",
    "std::net::",
    "core::net::",
    "std::path::",
];

/// Returns `ty` without the path to it in the standard library.
fn last_part(ty: &str) -> &str {
    let ty = ty.strip_prefix("::").unwrap_or(ty);
    PATHS
        .iter()
        .find_map(|path| ty.strip_prefix(path))
        .unwrap_or(ty)
}

/// Runs `$check` with each type the interpreter can check a conversion to.
macro_rules! checked_types {
    ($check:ident) => {
        $check!(
            "i8" => i8, "i16" => i16, "i32" => i32, "i64" => i64,
            "i128" => i128, "isize" => isize, "u8" => u8, "u16" => u16,
            "u32" => u32, "u64" => u64, "u128" => u128, "usize" => usize,
            "f32" => f32, "f64" => f64, "bool" => bool, "char" => char,
            "String" => String, "IpAddr" => std::net::IpAddr,
            "Ipv4Addr" => std::net::Ipv4Addr,
            "Ipv6Addr" => std::net::Ipv6Addr,
            "SocketAddr" => std::net::SocketAddr,
            "PathBuf" => std::path::PathBuf
        )
    };
}

/// Returns whether the interpreter can check a conversion to `ty`.
pub(super) fn checks(ty: &str) -> bool {
    macro_rules! names {
        ($($name:literal => $ty:ty),*) => {
            matches!(last_part(ty), $($name)|*)
        };
    }
    checked_types!(names)
}

/// Returns whether the type `ty` reads the text from `position` to `end`, as
/// the parser checks a conversion. A type the interpreter does not have takes
/// any text.
pub(super) fn accepts<const R: bool, W: Width>(
    parser: &mut Parser<'_, R, W>,
    rule: &Rule,
    ty: &str,
    position: usize,
    end: usize,
) -> bool {
    macro_rules! accept {
        ($($name:literal => $ty:ty),*) => {
            match last_part(ty) {
                $($name => parser.accepts::<$ty>(rule, position, end),)*
                _ => true,
            }
        };
    }
    checked_types!(accept)
}
