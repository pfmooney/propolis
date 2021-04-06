unsafe fn read_raw<T>(ptr: *const u8) -> T
where
    T: Copy + Default + Sized,
{
    let mut buf = T::default();
    std::ptr::copy_nonoverlapping(
        ptr,
        &mut buf as *mut T as *mut u8,
        std::mem::size_of::<T>(),
    );
    buf
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub(super) struct RawSubmission {
    pub cdw0: u32,
    pub nsid: u32,
    pub rsvd: u64,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}
impl RawSubmission {
    pub fn cid(&self) -> u16 {
        (self.cdw0 >> 16) as u16
    }
    pub fn opcode(&self) -> u8 {
        self.cdw0 as u8
    }
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub(super) struct RawCompletion {
    pub cdw0: u32,
    pub rsvd: u32,
    pub sqhd: u16,
    pub sqid: u16,
    pub cid: u16,
    pub status: u16,
}

// Register bits

pub const CAP_CCS: u64 = 1 << 37; // CAP.CCS - NVM command set
pub const CAP_CQR: u64 = 1 << 16; // CAP.CQR - require contiguous queus

pub const CC_EN: u32 = 0x1;

pub const CSTS_READY: u32 = 0x1;

// Version definitions

pub const NVME_VER_1_0: u32 = 0x00010000;

#[cfg(test)]
mod test {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn entry_sizing() {
        assert_eq!(size_of::<RawSubmission>(), 64);
        assert_eq!(size_of::<RawCompletion>(), 16);
    }
}
