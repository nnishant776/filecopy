mod copy;
use std::path::Path;

use clap::{App, Arg};
use copy::util as copyutils;

#[derive(Default, Debug)]
struct CmdlineCfg {
    src_path: String,
    dst_path: String,
    block_size: u64,
    progress: bool,
    statistics: bool,
    recursive: bool,
    force: bool,
    no_dir_err: bool,
    verbose: bool,
    remove: bool,
    resume: bool,
}

impl CmdlineCfg {
    fn new() -> Self {
        Self::default()
    }
}

fn main() {
    let cmdline_params = parse_cmdline_args();
    let copy_opts = cmdline_cfg_to_copy_opts(&cmdline_params);
    if let Err(e) = copy::copy(
        cmdline_params.src_path.as_str(),
        cmdline_params.dst_path.as_str(),
        copy_opts,
    ) {
        if cmdline_params.remove {
            println!("Move failed: {}", e);
        } else {
            println!("Copy failed: {}", e);
        }
        std::process::exit(1);
    }
}

fn parse_cmdline_args() -> CmdlineCfg {
    let mut cmdline_config_val = CmdlineCfg::new();

    let args_vec: Vec<String> = std::env::args().collect();

    let  cargs = App::new(Path::new(&args_vec[0].as_str()).file_name().unwrap().to_str().unwrap())
        .about("A file copy utility written in rust with progress and statistics tracking")
        .arg(
            Arg::new("block-size")
                .short('b')
                .long("block-size")
                .takes_value(true)
                .default_value("8M")
                .help("Block size for transfer (in units of K, M and G. Ex: 32M)"),
        )
        .arg(
            Arg::new("progress")
                .short('p')
                .long("progress")
                .help("Show progress of the transfer"),
        )
        .arg(
            Arg::new("recursive")
                .short('r')
                .long("recursive")
                .help("Copy files recursively"),
        )
        .arg(
            Arg::new("stats")
                .short('s')
                .long("stats")
                .help("Show statistics of the transfer"),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help("Overwrite the destination file"),
        )
        .arg(
            Arg::new("move")
                .short('m')
                .long("move")
                .help("Remove the source file after transfer"),
        )
        .arg(
            Arg::new("nodirerr")
                .short('n')
                .long("no-dir-error")
                .help("Ignore errors while copying directories"),
        )
        .arg(
            Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Print verbose output for the copy operation")
        )
        .arg(
            Arg::new("resume")
            .short('c')
            .long("continue")
            .help("Resume a partially completed copy")
        )
        .arg(Arg::new("SRC").help("Path to source file").required(true))
        .arg(Arg::new("DST").help("Path to destination").required(true))
        .after_help(
            "Supply source and destination respectively as positional arguments after specifying the options"
        );

    let matches = cargs.get_matches_from(args_vec);

    if let Some(blksize) = matches.value_of("block-size") {
        let block_size = copyutils::parse_size_from_str(blksize);
        cmdline_config_val.block_size = block_size;
    }

    cmdline_config_val.progress = matches.occurrences_of("progress") > 0;
    cmdline_config_val.recursive = matches.occurrences_of("recursive") > 0;
    cmdline_config_val.statistics = matches.occurrences_of("stats") > 0;
    cmdline_config_val.force = matches.occurrences_of("force") > 0;
    cmdline_config_val.remove = matches.occurrences_of("move") > 0;
    cmdline_config_val.no_dir_err = matches.occurrences_of("nodirerr") > 0;
    cmdline_config_val.verbose = matches.occurrences_of("verbose") > 0;
    cmdline_config_val.resume = matches.occurrences_of("resume") > 0;

    if let Some(src_path) = matches.value_of("SRC") {
        cmdline_config_val.src_path = src_path.to_owned();
    }

    if let Some(dst_path) = matches.value_of("DST") {
        cmdline_config_val.dst_path = dst_path.to_owned();
    }
    // println!("{:?}", &cmdline_config_val);
    cmdline_config_val
}

fn cmdline_cfg_to_copy_opts(cmdline_cfg: &CmdlineCfg) -> copy::CopyOptions {
    let mut copy_opts = copy::CopyOptions::new();

    copy_opts
        .block_size(cmdline_cfg.block_size)
        .force(cmdline_cfg.force)
        .recursive(cmdline_cfg.recursive)
        .progress(cmdline_cfg.progress)
        .remove(cmdline_cfg.remove)
        .stats(cmdline_cfg.statistics)
        .dircopy_err(cmdline_cfg.no_dir_err)
        .verbose(cmdline_cfg.verbose)
        .resume(cmdline_cfg.resume);

    copy_opts
}
