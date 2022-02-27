mod enums;
mod ioctls;
mod structs;
pub mod vmm_data;

pub use enums::*;
pub use ioctls::*;
pub use structs::*;
pub use vmm_data::*;

pub const VM_MAXCPU: usize = 32;

pub const VMM_PATH_PREFIX: &str = "/dev/vmm";
pub const VMM_CTL_PATH: &str = "/dev/vmmctl";
