use alloc::boxed::Box;

use spin::Once;

use super::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    NotMounted,
    InvalidPath,
    NotFound,
    IsDirectory,
    Io,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub(super) struct Inode(u64);

impl Inode {
    pub(super) fn new(inode: u64) -> Self {
        Self(inode)
    }

    pub(super) fn as_u64(&self) -> u64 {
        self.0
    }
}

pub(super) struct VirtualNode {
    pub(super) inode: Inode,
    pub(super) size: usize,
}

pub(super) trait FileSystem: Send + Sync {
    fn open(&self, path: &Path<'_>) -> Result<VirtualNode, Error>;
    fn read_at(&self, node: &VirtualNode, offset: usize, data: &mut [u8]) -> Result<usize, Error>;
}

static ROOT: Once<Box<dyn FileSystem>> = Once::new();

pub(super) fn mount_root(filesystem: impl FileSystem + 'static) {
    ROOT.call_once(|| Box::new(filesystem));
}

pub(crate) fn open(path: &str) -> Result<File, Error> {
    let path = Path::new(path).ok_or(Error::InvalidPath)?;
    let filesystem = ROOT.get().ok_or(Error::NotMounted)?.as_ref();
    let node = filesystem.open(&path)?;
    Ok(File { filesystem, node })
}

pub(crate) struct File {
    filesystem: &'static dyn FileSystem,
    node: VirtualNode,
}

impl File {
    pub(crate) fn len(&self) -> usize {
        self.node.size
    }

    /// Read at a byte offset, returning a short read at EOF.
    pub(crate) fn read_at(&self, offset: usize, data: &mut [u8]) -> Result<usize, Error> {
        self.filesystem.read_at(&self.node, offset, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test(target = "qemu")]
    fn test_read_init_from_root() {
        let file = open("/boot/init").unwrap();
        let mut magic = [0; 4];
        assert_eq!(file.read_at(0, &mut magic), Ok(magic.len()));
        assert_eq!(&magic, b"\x7fELF");
    }

    #[test(target = "qemu")]
    fn test_open_errors() {
        assert_eq!(open("boot/init").err(), Some(Error::InvalidPath));
        assert_eq!(open("/boot/MISSING.TST").err(), Some(Error::NotFound));
        assert_eq!(open("/").err(), Some(Error::IsDirectory));
        assert_eq!(open("/boot").err(), Some(Error::IsDirectory));
        assert_eq!(open("/boot/init/child").err(), Some(Error::NotFound));
    }

    #[test(target = "qemu")]
    fn test_read_offsets_and_eof() {
        let kernel = open("/boot/kernel").unwrap();
        let mut prefix = [0; 1050];
        assert_eq!(kernel.read_at(0, &mut prefix), Ok(prefix.len()));
        let mut across_sectors = [0; 40];
        assert_eq!(
            kernel.read_at(500, &mut across_sectors),
            Ok(across_sectors.len())
        );
        assert_eq!(&across_sectors, &prefix[500..540]);
        assert_eq!(
            kernel.read_at(1010, &mut across_sectors),
            Ok(across_sectors.len())
        );
        assert_eq!(&across_sectors, &prefix[1010..1050]);

        let file = open("/test/deep/inside/deepfile.txt").unwrap();
        let mut tail = [0; 4];
        assert_eq!(file.read_at(file.len() - 2, &mut tail), Ok(2));
        assert_eq!(&tail, b"h\n\0\0");
        assert_eq!(file.read_at(file.len(), &mut tail), Ok(0));
        assert_eq!(file.read_at(usize::MAX, &mut tail), Ok(0));
    }
}
