use super::util;
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    os::unix::prelude::{MetadataExt, OpenOptionsExt},
};
use std::{ops::Sub, path::Path};

#[derive(Clone)]
struct StatsStore {
    pub transferred: u64,
    pub total: u64,
    pub time_taken: std::time::Duration,
}

pub type ProgressHandler = fn(&Path, &Path, u64, u64, &CopyOptions);

#[derive(Clone)]
pub struct CopyOptions {
    block_size: u64,
    force: bool,
    show_progress: bool,
    recursive: bool,
    show_stats: bool,
    remove: bool,
    no_dir_err: bool,
    verbose: bool,
    resume: bool,
    progress_handler: Option<ProgressHandler>,
    stats_store: StatsStore,
}

#[allow(dead_code)]
impl CopyOptions {
    pub fn new() -> Self {
        Self {
            block_size: 8 * 1024 * 1024,
            force: false,
            show_progress: false,
            recursive: false,
            show_stats: false,
            remove: false,
            no_dir_err: false,
            verbose: false,
            resume: false,
            progress_handler: Some(default_progress_handler),
            stats_store: StatsStore {
                time_taken: std::time::Duration::from_secs(0),
                total: 0,
                transferred: 0,
            },
        }
    }

    pub fn block_size(&mut self, blk_size: u64) -> &mut Self {
        self.block_size = blk_size;
        self
    }

    pub fn force(&mut self, is_forced: bool) -> &mut Self {
        self.force = is_forced;
        self
    }

    pub fn progress(&mut self, show_progress: bool) -> &mut Self {
        self.show_progress = show_progress;
        self
    }

    pub fn recursive(&mut self, is_recursive: bool) -> &mut Self {
        self.recursive = is_recursive;
        self
    }

    pub fn remove(&mut self, remove_file: bool) -> &mut Self {
        self.remove = remove_file;
        self
    }

    pub fn stats(&mut self, show_stats: bool) -> &mut Self {
        self.show_stats = show_stats;
        self
    }

    pub fn progress_handler(&mut self, handler: ProgressHandler) -> &mut Self {
        self.progress_handler = Some(handler);
        self
    }

    pub fn dircopy_err(&mut self, ignore: bool) -> &mut Self {
        self.no_dir_err = ignore;
        self
    }

    pub fn verbose(&mut self, is_verbose: bool) -> &mut Self {
        self.verbose = is_verbose;
        self
    }

    pub fn resume(&mut self, is_resume: bool) -> &mut Self {
        self.resume = is_resume;
        self
    }
}

fn copy_directory(src: &Path, dst: &Path, copy_opts: &mut CopyOptions) -> Result<(), io::Error> {
    // get the list of all files under src recursively
    let filelist = util::list_dir_recursive_rel(Path::new(src))?;

    // calculate total bytes to be copied
    for fileinfo in &filelist {
        copy_opts.stats_store.total += fileinfo.size();
    }

    for fileinfo in &filelist {
        let cpy_src = src.join(fileinfo.path());
        let dst_src = dst.join(fileinfo.path());
        if let Err(e) = copy_file(cpy_src.as_path(), dst_src.as_path(), copy_opts) {
            if !copy_opts.no_dir_err {
                return Err(e);
            } else {
                println!("Failed to copy file: {}", &e);
            }
        } else if copy_opts.remove {
            if let Err(e) = std::fs::remove_file(&cpy_src) {
                if !copy_opts.no_dir_err {
                    return Err(io::Error::new(
                        e.kind(),
                        format!("failed to remove source file: {}", &e),
                    ));
                }
            }
        }
    }

    if copy_opts.remove {
        if let Err(e) = util::delete_dir_recursive(src) {
            return Err(io::Error::new(
                e.kind(),
                format!("failed to remove source directory: {}", &e),
            ));
        } else {
            return Ok(());
        }
    }

    Ok(())
}

/// copy copies `src` to `dst` based on the configuration options provded
/// in `copy_opts`.
pub fn copy(src: &str, dst: &str, copy_opts: CopyOptions) -> io::Result<()> {
    // if source and destination paths are same, abort copy
    if src == dst {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "destination is same as the source",
        ));
    }

    let mut copy_opts = copy_opts;

    let source = Path::new(src);
    let mut destination = Path::new(dst).to_owned();

    // check if the source path exists
    let src_stat = match std::fs::metadata(source) {
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("stat failed for source path: {}", &e),
            ))
        }
        Ok(s) => s,
    };

    // check for recursive copy
    if src_stat.is_dir() && !copy_opts.recursive {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source is a directory but --recursive option not specified",
        ));
    }

    // check if destination path exists
    if let Ok(dst_stat) = std::fs::metadata(dst) {
        if dst_stat.is_dir() {
            // if destination exists and is directory
            if let Some(basename) = source.file_name() {
                // set destination path as the original destination + basename
                // of the source path
                destination = destination.join(basename);
            }
        } else if src_stat.is_dir() {
            // if destination is a file but source is a directory, abort copy
            // with an error
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "source is a directory, destination is a file",
            ));
        }
    }

    // start timer
    let start = std::time::Instant::now();

    if src_stat.is_dir() {
        // if source is a directory, copy entire directory
        if let Err(e) = copy_directory(source, destination.as_path(), &mut copy_opts) {
            return Err(e);
        }
    } else {
        // if source is a file, copy the individual file
        copy_opts.stats_store.total = src_stat.len();
        if let Err(e) = copy_file(source, destination.as_path(), &mut copy_opts) {
            return Err(e);
        } else if copy_opts.remove {
            // if move option was specified, remove source file after
            // successful copy
            if let Err(e) = std::fs::remove_file(source) {
                return Err(io::Error::new(
                    e.kind(),
                    format!("failed to remove source file: {}", &e),
                ));
            }
        }
    }

    // stop timer
    let end = std::time::Instant::now();

    // verify copy stats
    if copy_opts.stats_store.transferred != copy_opts.stats_store.total {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "error in copy: transferred={}, total={}",
                &copy_opts.stats_store.transferred, &copy_opts.stats_store.total
            ),
        ));
    }

    // if statistics are requested, calculate and show the file transfer
    // statisctics
    if copy_opts.show_stats {
        copy_opts.stats_store.time_taken = end.sub(start);
        println!(
            "\nTime taken to copy: {:?}",
            copy_opts.stats_store.time_taken
        );
        let transfer_speed = (copy_opts.stats_store.total as f64
            / copy_opts.stats_store.time_taken.as_micros() as f64)
            as u64
            * 1_000_000;

        println!("Transfer speed: {}/s", get_str_size_precise(transfer_speed));
    }

    Ok(())
}

fn copy_file(src: &Path, dst: &Path, copy_opts: &mut CopyOptions) -> io::Result<usize> {
    // open the source file
    let mut src_file_handle = match File::open(src) {
        Ok(f) => f,
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("failure in opening source file: {}", e),
            ));
        }
    };

    // retrive source file metadata
    let src_file_metadata = match src_file_handle.metadata() {
        Ok(m) => m,
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("failure in fetching metadata for source file: {}", &e),
            ));
        }
    };

    // check if destination file exists
    let dst_file_metadata = match std::fs::metadata(dst) {
        Ok(m) => {
            // if destination file exists
            if !copy_opts.force && !copy_opts.resume {
                // if neither of force or resume option specified, abort copy
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "file '{}' exists, can't copy file without --force or --continue option",
                        dst.to_str().unwrap_or("")
                    ),
                ));
            }
            Some(m)
        }
        Err(_e) => {
            // if destination file doesn't exist
            if let Some(dst_dir) = dst.parent() {
                // create all the directories in the destination path
                if let Err(e) = std::fs::create_dir_all(dst_dir) {
                    // throw any error other than EEXIST
                    if e.kind() != io::ErrorKind::AlreadyExists {
                        return Err(io::Error::new(
                            e.kind(),
                            format!("failure in creating destination directory: {}", &e),
                        ));
                    }
                }
            }
            None
        }
    };

    // open the destination file
    let mut dst_file_handle: File = {
        let mut dst_file_open_options = std::fs::OpenOptions::new();

        dst_file_open_options.create(true).write(true);
        dst_file_open_options.mode(src_file_metadata.mode());

        if let Some(dst_file_meta) = &dst_file_metadata {
            if copy_opts.resume {
                // open in append mode if resume option is specified
                dst_file_open_options.append(true);
                dst_file_open_options.mode(dst_file_meta.mode());
            }
        }

        match dst_file_open_options.open(dst) {
            Ok(f) => f,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("failure in opening destination file: {}", &e),
                ));
            }
        }
    };

    let mut bytes_transferred: u64 = 0;

    if let Some(dst_file_meta) = &dst_file_metadata {
        // if destination file exists
        let dst_file_size = dst_file_meta.len();
        if copy_opts.resume {
            // if resume option is specified, skip the already copied bytes
            if let Err(e) = src_file_handle.seek(SeekFrom::Start(dst_file_size)) {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "failed to resume copy due to seek fail on source file: {}",
                        e
                    ),
                ));
            }

            // update transfer statistics
            bytes_transferred = dst_file_size;
            copy_opts.stats_store.transferred += dst_file_size;
        }
    }

    // specify progress logger
    let prgrs_hndlr = match copy_opts.progress_handler {
        Some(hndlr) => hndlr,
        None => default_progress_handler,
    };

    loop {
        match util::copy_n(
            &mut src_file_handle,
            &mut dst_file_handle,
            copy_opts.block_size as usize,
        ) {
            Ok(bytes_copied) => {
                // if 0 bytes were read or requested number of bytes were copied
                // successfully, exit loop
                if bytes_copied == 0 || bytes_transferred == src_file_metadata.len() {
                    break;
                }

                bytes_transferred += bytes_copied as u64;
                copy_opts.stats_store.transferred += bytes_copied as u64;

                // skip progress logging if not requested
                if !copy_opts.show_progress {
                    continue;
                }

                prgrs_hndlr(
                    src,
                    dst,
                    bytes_transferred,
                    src_file_metadata.len(),
                    copy_opts,
                );
            }
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "error while copying file '{}': {}",
                        &src.to_str().unwrap_or(""),
                        e
                    ),
                ))
            }
        }
    }

    // verify file transfer
    if bytes_transferred != src_file_metadata.len() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "error while copying file '{}': missing {} bytes in destination",
                &src.to_str().unwrap_or(""),
                src_file_metadata.len() - bytes_transferred
            ),
        ));
    }

    // sync permissions between source and destination files
    dst_file_handle.set_permissions(src_file_metadata.permissions())?;

    // print the final message about the file copy
    if copy_opts.show_progress {
        if copy_opts.remove {
            println!(
                "\rMoved file '{}'  ",
                &src.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new(""))
                    .to_str()
                    .unwrap_or("")
            );
        } else {
            println!(
                "\rCopied file '{}' ",
                &src.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new(""))
                    .to_str()
                    .unwrap_or("")
            );
        }
    }
    Ok(bytes_transferred as usize)
}

#[inline]
fn default_progress_handler(
    src: &Path,
    _dst: &Path,
    bytes_transferred: u64,
    total: u64,
    copy_opts: &CopyOptions,
) {
    let human_readable = true;
    let str_stats_transferred = get_str_size_precise(copy_opts.stats_store.transferred);
    let str_bytes_transferred = get_str_size_precise(bytes_transferred);
    let str_stats_total = get_str_size_precise(copy_opts.stats_store.total);
    let str_bytes_total = get_str_size_precise(total);

    if human_readable {
        print!(
            "\rCopying file {:50} ({:>8} /{:>8})\tTotal: ({:>8} /{:>8})",
            format!(
                "'{}'",
                src.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("/"))
                    .to_str()
                    .unwrap_or("")
            ),
            &str_bytes_transferred,
            &str_bytes_total,
            &str_stats_transferred,
            &str_stats_total,
        )
    } else {
        print!(
            "\rCopying file {:50} ({:8}/{:8})\tTotal: ({:10}/{:10})",
            format!("'{}'", src.to_str().unwrap_or("")),
            &bytes_transferred,
            &total,
            &copy_opts.stats_store.transferred,
            &copy_opts.stats_store.total,
        )
    }

    let _ = std::io::stdout().flush();
}

#[inline]
fn get_str_size_precise(bytes: u64) -> String {
    let result: String;
    if bytes > util::GB {
        result = format!("{:.2}G", (bytes as f64) / (util::GB as f64));
    } else if bytes > util::MB {
        result = format!("{:.2}M", (bytes as f64) / (util::MB as f64));
    } else if bytes > util::KB {
        result = format!("{:.2}K", (bytes as f64) / (util::KB as f64));
    } else {
        result = format!("{}B", bytes);
    }
    result
}
