// #[cfg(feature = "gpg")]
#[cfg(not(target_os = "windows"))]
pub mod gpg_generate;

#[cfg(target_os = "windows")]
pub mod gpg_generate_stub;

#[cfg(target_os = "windows")]
pub use gpg_generate_stub as gpg_generate;

