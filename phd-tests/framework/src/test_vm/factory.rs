//! Helpers for configuring and starting new VMs.

use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
    str::FromStr,
};

use anyhow::Result;
use thiserror::Error;
use tracing::info;

use crate::{
    artifacts::ArtifactStore, guest_os::GuestOsKind,
    test_vm::ServerProcessParameters,
};

use super::{vm_config, TestVm};

/// Errors that can arise while creating a VM factory.
#[derive(Debug, Error)]
pub enum FactoryConstructionError {
    /// Raised if the default bootrom key in the [`FactoryOptions`] does not
    /// yield a valid bootrom from the artifact store.
    #[error("Default bootrom {0} not in artifact store")]
    DefaultBootromMissing(String),

    /// Raised if the default guest image key in the [`FactoryOptions`] does not
    /// yield a valid image from the artifact store.
    #[error("Default guest image {0} not in artifact store")]
    DefaultGuestImageMissing(String),

    /// Raised on a failure to convert from a named server logging mode to a
    /// [`ServerLogMode`].
    #[error("Invalid server log mode name '{0}'")]
    InvalidServerLogModeName(String),
}

/// Specifies where propolis-server's log output should be written.
#[derive(Debug, Clone, Copy)]
pub enum ServerLogMode {
    /// Write to files in the server's factory's temporary directory.
    TmpFile,

    /// Write stdout/stderr to the console.
    Stdio,

    /// Redirect stdout/stderr to /dev/null.
    Null,
}

impl FromStr for ServerLogMode {
    type Err = FactoryConstructionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "file" | "tmpfile" => Ok(ServerLogMode::TmpFile),
            "stdio" => Ok(ServerLogMode::Stdio),
            "null" => Ok(ServerLogMode::Null),
            _ => Err(FactoryConstructionError::InvalidServerLogModeName(
                s.to_string(),
            )),
        }
    }
}

/// Parameters used to construct a new VM factory.
#[derive(Debug)]
pub struct FactoryOptions {
    /// The path to the Propolis server binary to use for VMs created by this
    /// factory.
    pub propolis_server_path: String,

    /// The directory to use as a temporary directory for config TOML files,
    /// server logs, and the like.
    pub tmp_directory: PathBuf,

    /// The logging discipline to use for this factory's Propolis servers.
    pub server_log_mode: ServerLogMode,

    /// An artifact store key specifying the default bootrom artifact to use for
    /// this factory's VMs.
    pub default_bootrom_artifact: String,

    /// An artifact store key specifying the default guest image artifact to use
    /// for this factory's VMs.
    pub default_guest_image_artifact: String,

    /// The default number of CPUs to set in [`vm_config::VmConfig`] structs
    /// generated by this factory.
    pub default_guest_cpus: u8,

    /// The default amount of memory to set in [`vm_config::VmConfig`] structs
    /// generated by this factory.
    pub default_guest_memory_mib: u64,
}

/// A VM factory that provides routines to generate new test VMs.
pub struct VmFactory {
    opts: FactoryOptions,
    default_guest_image_path: String,
    default_guest_kind: GuestOsKind,
    default_bootrom_path: String,
}

impl VmFactory {
    /// Creates a new VM factory with default bootrom/guest image artifacts
    /// drawn from the supplied artifact store.
    pub fn new(opts: FactoryOptions, store: &ArtifactStore) -> Result<Self> {
        info!(?opts, "Building VM factory");
        let (guest_path, kind) = store
            .get_guest_image_by_name(&opts.default_guest_image_artifact)
            .ok_or(FactoryConstructionError::DefaultGuestImageMissing(
                opts.default_guest_image_artifact.clone(),
            ))?;

        let bootrom_path = store
            .get_bootrom_by_name(&opts.default_bootrom_artifact)
            .ok_or(FactoryConstructionError::DefaultBootromMissing(
                opts.default_bootrom_artifact.clone(),
            ))?;

        Ok(Self {
            opts,
            default_guest_image_path: guest_path.to_string_lossy().to_string(),
            default_guest_kind: kind,
            default_bootrom_path: bootrom_path.to_string_lossy().to_string(),
        })
    }

    /// Creates a VM configuration that specifies this factory's defaults for
    /// CPUs, memory, bootrom, and guest image.
    ///
    /// The guest OS disk is attached as an NVMe disk in PCI slot 4.
    pub fn default_vm_config(&self) -> vm_config::VmConfigBuilder {
        self.deviceless_vm_config()
            .add_nvme_disk(&self.default_guest_image_path, 4)
    }

    /// Creates a VM configuration that specifies this factory's defaults for
    /// CPUs, memory, and bootrom.
    pub fn deviceless_vm_config(&self) -> vm_config::VmConfigBuilder {
        let bootrom_path =
            PathBuf::try_from(&self.default_bootrom_path).unwrap();
        vm_config::VmConfigBuilder::new()
            .set_bootrom_path(bootrom_path)
            .set_cpus(self.opts.default_guest_cpus)
            .set_memory_mib(self.opts.default_guest_memory_mib)
    }

    /// Launches a new Propolis server process with a VM configuration created
    /// by the supplied configuration builder. Returns the [`TestVm`] associated
    /// with this server.
    pub fn new_vm(
        &self,
        vm_name: &str,
        builder: vm_config::VmConfigBuilder,
    ) -> Result<TestVm> {
        let vm_config = builder.finish();
        info!(?vm_name, ?vm_config);

        let mut config_toml_path = self.opts.tmp_directory.clone();
        config_toml_path.push(format!("{}.config.toml", vm_name));
        vm_config.write_config_toml(&config_toml_path)?;

        let (server_stdout, server_stderr) = match &self.opts.server_log_mode {
            ServerLogMode::TmpFile => {
                let mut stdout_path = self.opts.tmp_directory.clone();
                stdout_path.push(format!("{}.stdout.log", vm_name));
                let mut stderr_path = self.opts.tmp_directory.clone();
                stderr_path.push(format!("{}.stderr.log", vm_name));
                info!(?stdout_path, ?stderr_path, "Opening server log files");
                (
                    std::fs::File::create(stdout_path)?.into(),
                    std::fs::File::create(stderr_path)?.into(),
                )
            }
            ServerLogMode::Stdio => {
                (std::process::Stdio::inherit(), std::process::Stdio::inherit())
            }
            ServerLogMode::Null => {
                (std::process::Stdio::null(), std::process::Stdio::null())
            }
        };

        let server_params = ServerProcessParameters {
            server_path: &self.opts.propolis_server_path,
            config_toml_path: &config_toml_path.as_os_str().to_string_lossy(),
            server_addr: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9000),
            server_stdout,
            server_stderr,
        };

        TestVm::new(vm_name, server_params, &vm_config, self.default_guest_kind)
    }
}