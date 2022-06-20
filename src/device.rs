use std::{borrow::Borrow, io, ops::Deref, path::PathBuf, rc::Rc, str::FromStr};

use libc::dev_t;

use crate::iter::{DevnoMapper, PartitionsIterator, SlavesIterator};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Devno(dev_t);

impl std::fmt::Debug for Devno {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Devno({}:{})", self.major(), self.minor())
    }
}

impl std::fmt::Display for Devno {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.major(), self.minor())
    }
}

impl Devno {
    pub fn major(&self) -> u32 {
        unsafe { libc::major(self.0) }
    }

    pub fn minor(&self) -> u32 {
        unsafe { libc::minor(self.0) }
    }
}

impl From<(u32, u32)> for Devno {
    fn from(raw: (u32, u32)) -> Self {
        Self(unsafe { libc::makedev(raw.0, raw.1) })
    }
}

impl From<dev_t> for Devno {
    #[inline]
    fn from(raw: dev_t) -> Self {
        Self(raw)
    }
}

impl Into<dev_t> for Devno {
    #[inline]
    fn into(self) -> dev_t {
        self.0
    }
}

impl Deref for Devno {
    type Target = dev_t;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<dev_t> for Devno {
    #[inline]
    fn as_ref(&self) -> &dev_t {
        &self.0
    }
}

impl Borrow<dev_t> for Devno {
    #[inline]
    fn borrow(&self) -> &dev_t {
        &self.0
    }
}

#[derive(Debug)]
pub struct ParseDevnoError;

impl FromStr for Devno {
    type Err = ParseDevnoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            let mut it = s.splitn(2, ':');
            let major: u32 = it.next().unwrap().parse().map_err(|_| ParseDevnoError)?;
            let minor: u32 = it
                .next()
                .map_or_else(|| Err(ParseDevnoError), |x| Ok(x))?
                .parse()
                .map_err(|_| ParseDevnoError)?;

            Ok(Self(unsafe { libc::makedev(major, minor) }))
        } else {
            s.parse::<libc::dev_t>()
                .map_err(|_| ParseDevnoError)
                .map(|x| Self(x))
        }
    }
}

#[derive(Clone)]
pub struct Device {
    blocks: Rc<crate::blocks::Blocks>,
    devno: Devno,
}

impl Device {
    #[inline]
    pub(crate) fn new(blocks: Rc<crate::blocks::Blocks>, devno: Devno) -> Self {
        Self { blocks, devno }
    }

    #[inline]
    pub fn is_partition(&self) -> io::Result<bool> {
        self.blocks.sysfs().is_partition(self)
    }

    #[inline]
    pub fn is_disk(&self) -> io::Result<bool> {
        self.is_partition().map(|x| !x)
    }

    #[inline]
    pub fn is_type(&self, ty: impl AsRef<str>) -> io::Result<bool> {
        self.blocks.is_type(self, ty)
    }

    #[inline]
    pub fn is_device_mapper(&self) -> io::Result<bool> {
        self.blocks.is_device_mapper(&self.devno)
    }

    #[inline]
    pub fn dm_uuid(&self) -> io::Result<Option<String>> {
        self.blocks.dm_uuid(&self.devno)
    }

    #[inline]
    pub fn dm_type(&self) -> io::Result<Option<String>> {
        self.blocks.dm_type(&self.devno)
    }

    #[inline]
    pub fn is_dm_types<S: AsRef<str>, I: IntoIterator<Item = S>>(
        &self,
        types: I,
    ) -> io::Result<bool> {
        self.blocks.is_dm_types(&self.devno, types)
    }

    #[inline]
    pub fn is_dm_type<S: AsRef<str>>(&self, t: S) -> io::Result<bool> {
        self.blocks.is_dm_type(&self.devno, t)
    }

    #[inline]
    pub fn is_luks(&self) -> io::Result<bool> {
        self.blocks.is_luks(&self.devno)
    }

    #[inline]
    pub fn is_luks2(&self) -> io::Result<bool> {
        self.blocks.is_luks2(&self.devno)
    }

    #[inline]
    pub fn partition_number(&self) -> io::Result<Option<usize>> {
        self.blocks.partition_number(&self.devno)
    }

    #[inline]
    pub fn slaves(&self) -> io::Result<DevnoMapper<SlavesIterator>> {
        let it = self.blocks.slaves(&self.devno)?;
        Ok(DevnoMapper::from_raw(&self.blocks, it))
    }

    #[inline]
    pub fn partitions(&self) -> io::Result<DevnoMapper<PartitionsIterator>> {
        let it = self.blocks.partitions(&self.devno)?;
        Ok(DevnoMapper::from_raw(&self.blocks, it))
    }

    #[inline]
    pub fn parent(&self) -> io::Result<Option<Self>> {
        self.blocks
            .parent(&self.devno)
            .map(|x| x.map(|devno| Self::new(self.blocks.clone(), devno)))
    }

    #[inline]
    pub fn to_devno(&self) -> Devno {
        self.devno
    }

    #[inline]
    pub fn path(&self) -> io::Result<PathBuf> {
        self.blocks.resolve(&self.devno)
    }

    #[inline]
    pub fn reread_partition_table(&self) -> io::Result<()> {
        self.blocks.reread_partition_table(&self.devno)
    }
}

impl std::fmt::Debug for Device {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.path() {
            Ok(path) => write!(f, "Device({:?})", path.display()),
            Err(err) => {
                println!("{:?}", err);
                write!(f, "Device({:?})", self.devno)
            }
        }
    }
}

impl PartialEq for Device {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.devno.eq(&other.devno)
    }
}

impl Eq for Device {}

impl PartialOrd for Device {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.devno.partial_cmp(&other.devno)
    }
}

impl Ord for Device {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.devno.cmp(&other.devno)
    }
}

impl std::hash::Hash for Device {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.devno.hash(state)
    }
}

impl Into<Devno> for Device {
    #[inline]
    fn into(self) -> Devno {
        self.devno
    }
}

impl Into<libc::dev_t> for Device {
    #[inline]
    fn into(self) -> libc::dev_t {
        self.devno.into()
    }
}

impl Deref for Device {
    type Target = Devno;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.devno
    }
}

impl AsRef<Devno> for Device {
    #[inline]
    fn as_ref(&self) -> &Devno {
        &self.devno
    }
}

impl AsRef<libc::dev_t> for Device {
    #[inline]
    fn as_ref(&self) -> &libc::dev_t {
        self.devno.as_ref()
    }
}

impl Borrow<Devno> for Device {
    #[inline]
    fn borrow(&self) -> &Devno {
        &self.devno
    }
}

impl Borrow<libc::dev_t> for Device {
    #[inline]
    fn borrow(&self) -> &libc::dev_t {
        self.devno.borrow()
    }
}
