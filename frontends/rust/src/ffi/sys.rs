//! The `hegel_*` C ABI as seen by the frontend: a re-export of the engine's
//! `hegel_c` crate, whose opaque handle types, `repr(C)` enums and structs,
//! and `hegel_*` functions are everything the frontend knows about libhegel.

pub(crate) use hegel_c::*;
