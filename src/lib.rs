pub mod devfs;
mod device;
pub mod iter;
pub mod procfs;
pub mod sysfs;
use std::{borrow::Borrow, io, path::Path, rc::Rc};

use devfs::DevFs;
pub use device::*;
use iter::DevnoMapper;
use procfs::{MountInfo, ProcFs};
use sysfs::{
    iter::{BlocksIterator, DisksIterator},
    SysFs,
};
pub(crate) mod blocks;

pub struct Blocks(Rc<blocks::Blocks>);

impl Blocks {
    #[inline]
    pub fn new() -> io::Result<Self> {
        Ok(Self(Rc::new(blocks::Blocks::new()?)))
    }

    #[inline]
    pub(crate) fn from_inner(inner: Rc<blocks::Blocks>) -> Self {
        Self(inner)
    }

    #[inline]
    pub fn procfs(&self) -> &ProcFs {
        self.0.procfs()
    }

    #[inline]
    pub fn sysfs(&self) -> &SysFs {
        self.0.sysfs()
    }

    #[inline]
    pub fn devfs(&self) -> &DevFs {
        self.0.devfs()
    }

    #[inline]
    pub fn from_path<P: AsRef<Path>>(&self, p: P) -> io::Result<Device> {
        Ok(Device::new(self.0.clone(), self.0.from_path(p)?))
    }

    #[inline]
    pub fn from_devno<D: Borrow<Devno>>(&self, d: D) -> io::Result<Device> {
        Ok(Device::new(self.0.clone(), self.0.from_devno(d)?))
    }

    #[inline]
    pub fn disks(&self) -> io::Result<DevnoMapper<DisksIterator>> {
        Ok(DevnoMapper::new(&self, self.0.disks()?))
    }

    #[inline]
    pub fn blocks(&self) -> io::Result<DevnoMapper<BlocksIterator>> {
        Ok(DevnoMapper::new(&self, self.0.blocks()?))
    }

    #[inline]
    pub fn mountinfo_from_path<P: AsRef<Path>>(&self, p: P) -> io::Result<MountInfo> {
        self.procfs().mountinfo_from_path(p)
    }
}

impl Clone for Blocks {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
