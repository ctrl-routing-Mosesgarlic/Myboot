//! Uniform port error type (ADR 0003; report §3.4). Adapters map firmware status
//! into these variants; the core matches on the variant, never a firmware code.
extern crate alloc;
use alloc::string::String;

/// A firmware variable namespace. The global namespace holds the UEFI-defined
/// boot variables; the vendor namespace is MyBoot's own private state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VarNamespace {
    /// EFI global variables, GUID 8BE4DF61-93CA-11D2-AA0D-00E098032B8C
    /// (Boot####, BootOrder, BootNext, BootCurrent).
    Global,
    /// MyBoot's vendor GUID — private persisted state (report §3.9).
    Vendor,
}

/// Uniform error for every port.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum IoError {
    NotFound,
    AccessDenied,
    Corrupt,
    DeviceError,
    /// A value was too large for the store (NVRAM is scarce).
    TooLarge,
    Other(String),
}

pub type IoResult<T> = Result<T, IoError>;
