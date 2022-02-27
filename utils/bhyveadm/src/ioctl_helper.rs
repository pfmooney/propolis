use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Result};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;

#[cfg(target_os = "illumos")]
pub fn ioctl<T>(fd: RawFd, cmd: i32, data: *mut T) -> Result<i32> {
    let res = unsafe { libc::ioctl(fd, cmd, data) };
    if res == -1 {
        Err(Error::last_os_error())
    } else {
        Ok(res)
    }
}
#[cfg(not(target_os = "illumos"))]
pub fn ioctl<T>(_fd: RawFd, _cmd: i32, _data: *mut T) -> Result<i32> {
    Err(Error::new(ErrorKind::Other, "illumos required"))
}

fn open_ctl() -> Result<File> {
    OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_EXCL)
        .open(bhyve_api::VMM_CTL_PATH)
}

pub fn create_vm(name: &str, flags: u64) -> Result<()> {
    let ctl = open_ctl()?;
    let fd = ctl.as_raw_fd();

    let mut create_arg = bhyve_api::vm_create_req::new(name);
    create_arg.flags = flags;

    ioctl(fd, bhyve_api::VMM_CREATE_VM, &mut create_arg)?;
    Ok(())
}
pub fn destroy_vm(name: &str) -> Result<()> {
    let ctl = open_ctl()?;
    let fd = ctl.as_raw_fd();

    let mut destroy_arg = bhyve_api::vm_destroy_req::new(name);

    ioctl(fd, bhyve_api::VMM_DESTROY_VM, &mut destroy_arg)?;
    Ok(())
}

pub struct VmmHdl(File);
impl VmmHdl {
    pub fn open(name: &str) -> Result<Self> {
        let mut vmpath = PathBuf::from(bhyve_api::VMM_PATH_PREFIX);
        vmpath.push(name);
        let fp = OpenOptions::new().write(true).read(true).open(vmpath)?;

        Ok(Self(fp))
    }
    pub fn ioctl<T>(&self, cmd: i32, data: *mut T) -> Result<i32> {
        ioctl(self.0.as_raw_fd(), cmd, data)
    }

    pub fn get_data_raw<T>(
        &self,
        vcpuid: i32,
        class: bhyve_api::VmmDataClass,
        version: u16,
        flags: u32,
        data: &mut T,
    ) -> Result<()>
    where
        T: Sized,
    {
        assert!(
            vcpuid == -1
                || (vcpuid >= 0 && vcpuid < bhyve_api::VM_MAXCPU as i32)
        );
        let len = std::mem::size_of::<T>();

        let mut arg = bhyve_api::vm_data_xfer {
            vdx_vcpuid: vcpuid,
            vdx_class: class as u16,
            vdx_version: version,
            vdx_flags: flags,
            vdx_len: len as u32,
            vdx_data: data as *mut T as *mut libc::c_void,
        };
        let _ = self.ioctl(bhyve_api::VM_DATA_READ, &mut arg)?;
        Ok(())
    }

    pub fn get_data<T>(
        &self,
        vcpuid: i32,
        class: bhyve_api::VmmDataClass,
        version: u16,
        flags: u32,
    ) -> Result<T>
    where
        T: Sized + Copy + Default,
    {
        let mut buf = T::default();
        self.get_data_raw(vcpuid, class, version, flags, &mut buf)?;

        Ok(buf)
    }
}
