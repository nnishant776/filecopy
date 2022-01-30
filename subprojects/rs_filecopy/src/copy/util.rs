use std::{
    fs::{self, File},
    io,
    io::{Read, Write},
    path::Path,
};

pub(crate) const KB: u64 = 1024;
pub(crate) const MB: u64 = 1024 * KB;
pub(crate) const GB: u64 = 1024 * MB;

#[derive(Debug)]
pub(crate) struct DirFile {
    path: String,
    size: u64,
}
impl DirFile {
    pub(crate) fn size(&self) -> u64 {
        self.size
    }
    pub(crate) fn path(&self) -> &String {
        &self.path
    }
}

pub(crate) fn delete_dir_recursive(basepath: &Path) -> io::Result<()> {
    for entry in fs::read_dir(basepath)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            delete_dir_recursive(entry.path().as_path())?;
        }
    }
    if let Err(e) = fs::remove_dir(basepath) {
        if io::ErrorKind::NotFound != e.kind() {
            return Err(e);
        }
    }
    Ok(())
}

/// Given a path, it generates a list of file paths and the file size
/// recursively. It returns any error thrown by [`std::fs::read_dir`] or
/// [`std::fs::DirEntry::metadata`] with some extra message to give context
/// of what went wrong. The [`io::ErrorKind`] value remains the same.
pub(crate) fn list_dir_recursive_rel(basepath: &Path) -> Result<Vec<DirFile>, io::Error> {
    list_dir_recursive_rel_util(basepath, Path::new(""))
}

fn list_dir_recursive_rel_util(basepath: &Path, abspath: &Path) -> Result<Vec<DirFile>, io::Error> {
    let mut result = Vec::<DirFile>::new();
    let read_path = basepath.join(abspath);
    let dir_reader = match std::fs::read_dir(&read_path.as_path()) {
        Ok(r) => r,
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "failure in reading directory '{}': {}",
                    &read_path.to_str().unwrap_or(""),
                    &e
                ),
            ));
        }
    };
    for entry in dir_reader {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("failure in reading directory entry: {}", e),
                ));
            }
        };
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "failure in reading metadata entry for file '{}': {}",
                        &entry.path().to_str().unwrap_or(""),
                        e
                    ),
                ));
            }
        };
        let path = abspath.join(&entry.file_name());
        if metadata.is_dir() {
            if let Ok(mut filelist) = list_dir_recursive_rel_util(basepath, path.as_path()) {
                result.append(&mut filelist);
            }
        } else {
            result.push(DirFile {
                path: String::from(path.as_path().to_str().unwrap_or("")),
                size: metadata.len(),
            });
        }
    }
    Ok(result)
}

/// Parsee a human readable size to bytes. In case of an error, it returns
/// byte value of 8M, i.e., 8 * 1024 * 1024 bytes
pub(crate) fn parse_size_from_str(str_size: &str) -> u64 {
    let str_size_bytes = str_size.as_bytes();
    let mut i = 0;
    for x in str_size_bytes {
        if (b'0'..=b'9').contains(x) {
            i += 1
        } else {
            break;
        }
    }

    let (size_num, size_suffix) = (
        String::from_utf8(str_size_bytes[..i].to_vec())
            .unwrap_or_else(|e| {
                println!("found invalid utf-8 size string: {}", e);
                "8".to_string()
            })
            .parse::<u64>()
            .unwrap_or(8),
        String::from_utf8(str_size_bytes[i..].to_vec()).unwrap_or_else(|e| {
            println!("found invalid utf-8 size suffix string: {}", e);
            "M".to_string()
        }),
    );
    match size_suffix.as_str() {
        "k" | "K" => size_num * KB,
        "m" | "M" => size_num * MB,
        "g" | "G" => size_num * GB,
        _ => 8 * MB,
    }
}

/// Copies upto `bytes_to_read` bytes of data from `src` to `dst`. Returns
/// the total number of bytes actually transferred or an error if it occurs.
pub(crate) fn copy_n(src: &mut File, dst: &mut File, bytes_to_read: usize) -> io::Result<usize> {
    const DEFAULT_BUFFER_SIZE: usize = 32 * KB as usize;
    let mut bytes_to_read_local = bytes_to_read;
    let mut buf = [0u8; DEFAULT_BUFFER_SIZE];
    loop {
        let remaining_bytes = min(bytes_to_read_local as u64, DEFAULT_BUFFER_SIZE as u64) as usize;
        match src.read(&mut buf[..remaining_bytes]) {
            Ok(read_cnt) => {
                if read_cnt == 0 || bytes_to_read_local == 0 {
                    break;
                }
                bytes_to_read_local -= read_cnt;
                dst.write_all(&buf[..read_cnt])?;
            }
            Err(_e) => {
                break;
            }
        }
    }
    Ok(bytes_to_read - bytes_to_read_local)
}

fn min(a: u64, b: u64) -> u64 {
    if a < b {
        return a;
    }
    b
}
