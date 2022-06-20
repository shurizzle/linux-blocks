pub mod iter;

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{procfs::ProcFs, Devno};

use self::iter::{BlocksIterator, DisksIterator};

pub struct SysFs {
    path: PathBuf,
}

impl SysFs {
    pub fn new(procfs: &ProcFs) -> io::Result<Self> {
        let path = match procfs.mounts().find(|m| {
            m.file_system == "sysfs" && matches!(m.source.as_ref().map(|x| x.as_str()), Some("sys"))
        })? {
            Some(pp) => pp.mount_point,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "sysfs not found")),
        };
        Ok(Self { path })
    }

    pub fn resolve(&self, devno: &Devno) -> io::Result<PathBuf> {
        let path = self.path.join("dev").join("block").join(devno.to_string());
        if path.exists() {
            path.canonicalize()
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }

    pub fn dm_name(&self, devno: &Devno) -> io::Result<Option<String>> {
        let p = self.resolve(devno)?;
        let dmname_path = p.join("dm").join("name");
        if dmname_path.exists() {
            Ok(Some(
                std::fs::read_to_string(dmname_path)?.trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    pub fn dm_uuid(&self, devno: &Devno) -> io::Result<Option<String>> {
        let p = self.resolve(devno)?;
        let dmuuid_path = p.join("dm").join("uuid");
        if dmuuid_path.exists() {
            Ok(Some(
                std::fs::read_to_string(dmuuid_path)?.trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    pub fn dm_type(&self, devno: &Devno) -> io::Result<Option<String>> {
        let t = match self.dm_uuid(devno)? {
            Some(t) => t,
            None => return Ok(None),
        };
        let mut it = t.split('-');
        let mut res = String::new();
        {
            let mut first = it.next().unwrap();
            if first.starts_with("part") {
                match it.next() {
                    Some(x) => first = x,
                    None => return Err(io::ErrorKind::InvalidData.into()),
                }
            }
            res.push_str(first);
            res.push('-');
        }

        match it.next() {
            Some(x) => res.push_str(x),
            None => return Err(io::ErrorKind::InvalidData.into()),
        }

        Ok(Some(res))
    }

    pub fn partition_number(&self, devno: &Devno) -> io::Result<Option<usize>> {
        let p = self.resolve(devno)?;

        let ppath = p.join("partition");
        if ppath.exists() {
            match std::fs::read_to_string(ppath)?.trim().parse::<usize>() {
                Ok(partno) => Ok(Some(partno)),
                Err(_) => Err(io::ErrorKind::InvalidData.into()),
            }
        } else {
            let dm_uuid = match self.dm_uuid(devno)? {
                Some(u) => u,
                None => return Ok(None),
            };

            if dm_uuid.starts_with("part") {
                match dm_uuid[4..].splitn(2, '-').next().unwrap().parse::<usize>() {
                    Ok(partno) => Ok(Some(partno)),
                    Err(_) => Err(io::ErrorKind::InvalidData.into()),
                }
            } else {
                Ok(None)
            }
        }
    }

    #[inline]
    pub fn is_partition(&self, devno: &Devno) -> io::Result<bool> {
        self.partition_number(devno).map(|x| x.is_some())
    }

    #[inline]
    pub fn is_wholedisk(&self, devno: &Devno) -> io::Result<bool> {
        self.is_partition(devno).map(|x| !x)
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[inline]
    pub fn disks(&self) -> io::Result<DisksIterator> {
        DisksIterator::new(&self)
    }

    #[inline]
    pub fn blocks(&self) -> io::Result<BlocksIterator> {
        BlocksIterator::new(&self)
    }
}
