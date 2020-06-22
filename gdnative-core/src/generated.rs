#![allow(non_snake_case)] // because of the generated bindings.
#![allow(unused_imports)]
#![allow(unused_unsafe)]
// False positives on generated drops that enforce lifetime
#![allow(clippy::drop_copy)]
// Disable non-critical lints for generated code.
#![allow(clippy::style, clippy::complexity, clippy::perf)]

use super::*;
use crate as gdnative_core;
use crate::sys::GodotApi;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
