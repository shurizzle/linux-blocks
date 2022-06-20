use std::{
    fs::File,
    io::{self, BufRead, BufReader, Lines},
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::Devno;

#[derive(Debug, Clone)]
pub struct MountInfo {
    pub id: u32,
    pub parent_id: u32,
    pub dev: Devno,
    pub root: PathBuf,
    pub mount_point: PathBuf,
    pub mount_options: String,
    pub fields: Vec<String>,
    pub file_system: String,
    pub source: Option<String>,
    pub super_options: String,
}

#[derive(Debug)]
pub struct ParseMountInfoError;

impl FromStr for MountInfo {
    type Err = ParseMountInfoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut it = s.split_whitespace();

        Ok(Self {
            id: it
                .next()
                .unwrap()
                .parse()
                .map_err(|_| ParseMountInfoError)?,
            parent_id: it
                .next()
                .ok_or(ParseMountInfoError)?
                .parse()
                .map_err(|_| ParseMountInfoError)?,
            dev: it
                .next()
                .ok_or(ParseMountInfoError)?
                .parse()
                .map_err(|_| ParseMountInfoError)?,
            root: it.next().ok_or(ParseMountInfoError)?.into(),
            mount_point: it.next().ok_or(ParseMountInfoError)?.into(),
            mount_options: it.next().ok_or(ParseMountInfoError)?.into(),
            fields: {
                let mut fields = Vec::new();
                loop {
                    let v = it.next().ok_or(ParseMountInfoError)?;
                    if v == "-" {
                        break;
                    } else {
                        fields.push(v.to_string());
                    }
                }
                fields
            },
            file_system: it.next().ok_or(ParseMountInfoError)?.to_string(),
            source: {
                let source = it.next().ok_or(ParseMountInfoError)?;
                if source == "none" {
                    None
                } else {
                    Some(source.to_string())
                }
            },
            super_options: it.next().ok_or(ParseMountInfoError)?.to_string(),
        })
    }
}

impl std::fmt::Display for MountInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} ",
            self.id,
            self.parent_id,
            self.dev,
            self.root.display(),
            self.mount_point.display(),
            self.mount_options
        )?;

        for field in self.fields.iter() {
            write!(f, "{} ", field)?;
        }

        write!(
            f,
            "- {} {} {}",
            self.file_system,
            self.source.as_ref().map(|x| x.as_str()).unwrap_or("none"),
            self.super_options
        )
    }
}

pub struct MountInfoIterator {
    lines: Lines<BufReader<File>>,
}

impl MountInfoIterator {
    pub fn new<P: AsRef<Path>>(file: P) -> io::Result<Self> {
        Ok(Self {
            lines: BufReader::new(File::open(file)?).lines(),
        })
    }

    #[inline]
    pub fn from_procfs<P: AsRef<Path>>(procfs: P) -> io::Result<Self> {
        Self::new(procfs.as_ref().join("self").join("mountinfo"))
    }
}

impl Iterator for MountInfoIterator {
    type Item = io::Result<MountInfo>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.lines.next()? {
            Ok(line) => match line.parse() {
                Ok(info) => Some(Ok(info)),
                Err(_) => Some(Err(io::ErrorKind::InvalidData.into())),
            },
            Err(err) => Some(Err(err)),
        }
    }
}

pub struct MountInfos {
    path: PathBuf,
}

impl MountInfos {
    #[inline]
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    #[inline]
    pub fn from_procfs<P: AsRef<Path>>(procfs: P) -> Self {
        Self::new(procfs.as_ref().join("self").join("mountinfo"))
    }

    pub fn find<F: Fn(&MountInfo) -> bool>(&self, f: F) -> io::Result<Option<MountInfo>> {
        for mount in self.iter()? {
            let mount = mount?;
            if f(&mount) {
                return Ok(Some(mount));
            }
        }
        Ok(None)
    }

    #[inline]
    pub fn iter(&self) -> io::Result<MountInfoIterator> {
        MountInfoIterator::new(&self.path)
    }

    #[inline]
    pub fn all(&self) -> io::Result<Vec<MountInfo>> {
        self.iter()?.collect()
    }
}
