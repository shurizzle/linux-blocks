mod devices;
mod mountinfo;

use std::{
    ffi::CString,
    io,
    os::unix::prelude::{MetadataExt, OsStrExt},
    path::{Path, PathBuf},
};

pub use devices::*;
pub use mountinfo::*;

use crate::Devno;

pub struct ProcFs {
    path: PathBuf,
    devices: Devices,
    mounts: MountInfos,
}

impl ProcFs {
    #[inline]
    pub fn new() -> io::Result<Self> {
        Self::remap(Self::probe()?)
    }

    #[inline]
    pub fn with_path<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        Self::remap(Self::probe_by_path(p)?)
    }

    #[inline]
    pub fn with_paths<P: AsRef<Path>, I: IntoIterator<Item = P>>(it: I) -> io::Result<Self> {
        Self::remap(Self::probe_by_paths(it)?)
    }

    #[inline]
    pub fn devices(&self) -> &Devices {
        &self.devices
    }

    #[inline]
    pub fn mounts(&self) -> &MountInfos {
        &self.mounts
    }

    pub fn mountinfo_from_path<P: AsRef<Path>>(&self, p: P) -> io::Result<MountInfo> {
        let dev: Devno = {
            let md = p.as_ref().metadata()?;
            md.dev().into()
        };

        let mount_id = {
            let path =
                unsafe { CString::from_vec_unchecked(p.as_ref().as_os_str().as_bytes().to_vec()) };
            let mut handle = file_handle {
                handle_bytes: 0,
                handle_type: 0,
                f_handle: [],
            };
            let mut mount_id = 0;
            let ret = unsafe {
                name_to_handle_at(
                    libc::AT_FDCWD,
                    path.as_ptr(),
                    &mut handle,
                    &mut mount_id,
                    libc::AT_EMPTY_PATH,
                )
            };

            if ret < 0 && io::Error::last_os_error().raw_os_error().unwrap_or(0) != libc::EOVERFLOW
            {
                None
            } else {
                Some(mount_id as u32)
            }
        };

        self.mounts()
            .find(|info| mount_id.map(|id| info.id == id).unwrap_or(false) || info.dev == dev)?
            .ok_or(io::ErrorKind::NotFound.into())
    }

    #[inline]
    pub fn is_type(&self, devno: &Devno, ty: impl AsRef<str>) -> io::Result<bool> {
        Ok(&*match self.devices().get_by_id(devno.major())? {
            Some(x) => x,
            None => return Ok(false),
        } == ty.as_ref())
    }

    #[inline]
    fn remap(p: Option<PathBuf>) -> io::Result<Self> {
        match p {
            Some(path) => Ok(Self {
                devices: Devices::new(&path),
                mounts: MountInfos::from_procfs(&path),
                path,
            }),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    fn probe_by_mtab() -> io::Result<Option<PathBuf>> {
        let p = Path::new("/etc/mtab");
        if !p.is_symlink() {
            return Ok(None);
        }
        let p = p.canonicalize()?;
        let parent = match p.parent() {
            Some(parent) => parent,
            None => return Ok(None),
        };
        if match parent.file_name() {
            Some(name) => name,
            None => return Ok(None),
        } != "self"
        {
            return Ok(None);
        }
        let procfs = match parent.parent() {
            Some(procfs) => procfs,
            None => return Ok(None),
        };
        if matches!(procfs.file_name(), Some(_)) {
            Self::probe_by_path(procfs)
        } else {
            Ok(None)
        }
    }

    fn probe_by_path<P: AsRef<Path>>(p: P) -> io::Result<Option<PathBuf>> {
        let p = p.as_ref().canonicalize()?;

        if p.join("self").canonicalize()? == p.join(std::process::id().to_string()) {
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    fn probe_by_paths<P: AsRef<Path>, I: IntoIterator<Item = P>>(
        it: I,
    ) -> io::Result<Option<PathBuf>> {
        for p in it {
            match Self::probe_by_path(p)? {
                Some(path) => return Ok(Some(path)),
                _ => (),
            }
        }
        Ok(None)
    }

    fn probe() -> io::Result<Option<PathBuf>> {
        if let Some(p) = Self::probe_by_mtab()? {
            return Ok(Some(p));
        }
        Self::probe_by_path("/proc")
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[repr(C)]
pub struct file_handle {
    pub handle_bytes: libc::c_uint,
    pub handle_type: libc::c_int,
    pub f_handle: [libc::c_uchar; 0],
}

extern "C" {
    fn name_to_handle_at(
        dirfd: libc::c_int,
        pathname: *const libc::c_char,
        handle: *mut file_handle,
        mount_id: *mut libc::c_int,
        flags: libc::c_int,
    ) -> libc::c_int;
}
