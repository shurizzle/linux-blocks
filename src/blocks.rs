use std::{
    borrow::Borrow,
    fs::OpenOptions,
    io,
    os::unix::prelude::{AsRawFd, FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use crate::{
    devfs::DevFs,
    iter,
    procfs::ProcFs,
    sysfs::{
        iter::{BlocksIterator, DisksIterator},
        SysFs,
    },
    Devno,
};

pub(crate) struct Blocks {
    procfs: ProcFs,
    sysfs: SysFs,
    devfs: DevFs,
}

const BLKRRPART: u64 = 4703;

impl Blocks {
    pub fn new() -> io::Result<Self> {
        let procfs = ProcFs::new()?;
        let sysfs = SysFs::new(&procfs)?;
        let devfs = DevFs::new(&procfs)?;
        Ok(Self {
            procfs,
            sysfs,
            devfs,
        })
    }

    #[inline]
    pub fn sysfs(&self) -> &SysFs {
        &self.sysfs
    }

    #[inline]
    pub fn procfs(&self) -> &ProcFs {
        &self.procfs
    }

    #[inline]
    pub fn devfs(&self) -> &DevFs {
        &self.devfs
    }

    #[inline]
    pub fn is_type(&self, devno: &Devno, ty: impl AsRef<str>) -> io::Result<bool> {
        self.procfs().is_type(devno, ty)
    }

    #[inline]
    pub fn is_device_mapper(&self, devno: &Devno) -> io::Result<bool> {
        self.is_type(devno, "device-mapper")
    }

    #[inline]
    pub fn dm_uuid(&self, devno: &Devno) -> io::Result<Option<String>> {
        self.sysfs().dm_uuid(devno)
    }

    #[inline]
    pub fn dm_type(&self, devno: &Devno) -> io::Result<Option<String>> {
        self.sysfs().dm_type(devno)
    }

    pub fn is_dm_types<S: AsRef<str>, I: IntoIterator<Item = S>>(
        &self,
        devno: &Devno,
        types: I,
    ) -> io::Result<bool> {
        if self.is_device_mapper(devno)? {
            match self.dm_type(devno)? {
                Some(t) => {
                    for t1 in types {
                        println!("{} == {} -> {:?}", t, t1.as_ref(), t == t1.as_ref());
                        if t == t1.as_ref() {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                None => return Err(io::ErrorKind::InvalidData.into()),
            }
        } else {
            Ok(false)
        }
    }

    #[inline]
    pub fn is_dm_type<S: AsRef<str>>(&self, devno: &Devno, t: S) -> io::Result<bool> {
        self.is_dm_types(devno, &[t])
    }

    #[inline]
    pub fn is_luks(&self, devno: &Devno) -> io::Result<bool> {
        match self.dm_type(devno)? {
            Some(x) => Ok(x.starts_with("CRYPT-LUKS")),
            None => Ok(false),
        }
    }

    #[inline]
    pub fn is_luks2(&self, devno: &Devno) -> io::Result<bool> {
        self.is_dm_type(devno, "CRYPT-LUKS2")
    }

    #[inline]
    pub fn partition_number(&self, devno: &Devno) -> io::Result<Option<usize>> {
        self.sysfs().partition_number(devno)
    }

    #[inline]
    pub fn is_partition(&self, devno: &Devno) -> io::Result<bool> {
        self.sysfs().is_partition(devno)
    }

    #[inline]
    pub fn is_disk(&self, devno: &Devno) -> io::Result<bool> {
        self.is_partition(devno).map(|x| !x)
    }

    pub fn partitions<'a>(&'a self, devno: &Devno) -> io::Result<iter::PartitionsIterator<'a>> {
        if self.is_disk(devno)? {
            let path = self.sysfs().resolve(devno)?;

            if self.is_type(devno, "device-mapper")? {
                if self.is_luks(devno)? {
                    iter::PartitionsIterator::masters(&self, *devno)
                } else {
                    iter::PartitionsIterator::empty()
                }
            } else {
                iter::PartitionsIterator::partitions(path)
            }
        } else {
            iter::PartitionsIterator::empty()
        }
    }

    pub fn slaves(&self, devno: &Devno) -> io::Result<iter::SlavesIterator> {
        let path = self.sysfs().resolve(devno)?;

        if self.is_luks(devno)? {
            iter::SlavesIterator::empty()
        } else {
            iter::SlavesIterator::new(path)
        }
    }

    pub fn parent(&self, devno: &Devno) -> io::Result<Option<Devno>> {
        if self.is_disk(devno)? {
            if self.is_luks(devno)? {
                match iter::RawSlavesIterator::new(self.sysfs().resolve(devno)?.join("slaves"))?
                    .next()
                {
                    Some(x) => Ok(Some(x?)),
                    None => Err(io::ErrorKind::NotFound.into()),
                }
            } else {
                Ok(None)
            }
        } else {
            if self.is_luks(devno)? {
                match iter::RawSlavesIterator::new(self.sysfs().resolve(devno)?.join("slaves"))?
                    .next()
                {
                    Some(x) => Ok(Some(x?)),
                    None => Err(io::ErrorKind::NotFound.into()),
                }
            } else {
                match self.sysfs().resolve(devno)?.parent() {
                    Some(d) => match std::fs::read_to_string(d.join("dev"))?
                        .trim()
                        .parse::<Devno>()
                    {
                        Ok(devno) => Ok(Some(devno)),
                        Err(_) => Err(io::ErrorKind::InvalidData.into()),
                    },
                    None => Err(io::ErrorKind::NotFound.into()),
                }
            }
        }
    }

    pub fn from_path<P: AsRef<Path>>(&self, p: P) -> io::Result<Devno> {
        let md = p.as_ref().metadata()?;
        if md.file_type().is_block_device() {
            Ok(md.rdev().into())
        } else {
            Err(io::ErrorKind::InvalidInput.into())
        }
    }

    #[inline]
    pub fn from_devno<B: Borrow<Devno>>(&self, d: B) -> io::Result<Devno> {
        let devno = d.borrow();
        let _ = self.devfs().resolve(devno)?;
        Ok(*devno)
    }

    #[inline]
    pub fn resolve<B: Borrow<Devno>>(&self, d: B) -> io::Result<PathBuf> {
        let devno = d.borrow();
        if self.is_device_mapper(devno)? {
            let name = match self.sysfs().dm_name(devno)? {
                Some(n) => n,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Device mapper has no dm name",
                    ))
                }
            };
            let dmpath = self.devfs().path().join("mapper").join(name);

            if dmpath.exists() {
                return Ok(dmpath);
            }
        }

        self.devfs().resolve(devno)
    }

    #[inline]
    pub fn disks(&self) -> io::Result<DisksIterator> {
        self.sysfs().disks()
    }

    #[inline]
    pub fn blocks(&self) -> io::Result<BlocksIterator> {
        self.sysfs().blocks()
    }

    pub fn reread_partition_table(&self, devno: &Devno) -> io::Result<()> {
        let p = self.devfs().resolve(devno)?;
        let f = OpenOptions::new()
            .read(true)
            .write(false)
            .create_new(false)
            .truncate(false)
            .create(false)
            .append(false)
            .open(p)?;
        let ret = unsafe { libc::ioctl(f.as_raw_fd() as _, BLKRRPART) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
