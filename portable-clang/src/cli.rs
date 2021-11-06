// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    anyhow::{anyhow, Context, Result},
    clap::{App, AppSettings, Arg, ArgMatches, SubCommand},
    slog::Logger,
    std::path::PathBuf,
};

const PCLANG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tool names that the clang frontend responds to.
const CLANG_TOOLS: &[&str] = &[
    "clang",
    "clang++",
    "clang-c++",
    "clang-cc",
    "clang-cpp",
    "clang-g++",
    "clang-gcc",
    "clang-cl",
    "cc",
    "cpp",
    "cl",
    "++",
    "flang",
];

pub fn run() -> Result<i32> {
    let exe = std::env::current_exe().context("resolving current executable")?;

    if let Some(stem) = exe.file_stem() {
        if CLANG_TOOLS.contains(&stem.to_string_lossy().as_ref()) {
            println!("running as clang tool {}", stem.to_string_lossy());
            return Ok(0);
        }
    }

    run_pclang()
}

/// Run the main `pclang` CLI.
pub fn run_pclang() -> Result<i32> {
    let logger = crate::logging::logger();

    let app = App::new("pclang")
        .setting(AppSettings::ArgRequiredElseHelp)
        .version(PCLANG_VERSION)
        .author("Gregory Szorc <gregory.szorc@gmail.com>");

    let app = app.subcommand(
        SubCommand::with_name("fetch-gcc-sources")
            .about("Download GCC source tarballs")
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Directory to write files to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("fetch-support")
            .about("Fetch support artifacts needed to build")
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Directory to write files to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("fetch-llvm-sources")
            .about("Download LLVM source tarballs")
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Directory to write files to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("fetch-secure")
            .about("Download a URL while checking its SHA-256")
            .arg(Arg::with_name("url").required(true).help("URL to download"))
            .arg(
                Arg::with_name("sha256")
                    .required(true)
                    .help("SHA-256 of downloaded content"),
            )
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Destination filename to write file to"),
            ),
    );

    let matches = app.get_matches();

    match matches.subcommand() {
        ("fetch-gcc-sources", Some(args)) => command_fetch_gcc_sources(&logger, args),
        ("fetch-llvm-sources", Some(args)) => command_fetch_llvm_sources(&logger, args),
        ("fetch-secure", Some(args)) => command_fetch_secure(&logger, args),
        ("fetch-support", Some(args)) => command_fetch_support(&logger, args),
        _ => Err(anyhow!("invalid sub-command")),
    }
}

fn command_fetch_gcc_sources(logger: &Logger, args: &ArgMatches) -> Result<i32> {
    let dest = args.value_of("dest").expect("dest argument is required");

    let dest = PathBuf::from(dest);

    crate::downloads::fetch_gcc_sources(logger, &dest).context("fetching GCC sources")?;

    Ok(0)
}

fn command_fetch_llvm_sources(logger: &Logger, args: &ArgMatches) -> Result<i32> {
    let dest = args.value_of("dest").expect("dest argument is required");

    let dest = PathBuf::from(dest);

    crate::downloads::fetch_llvm_sources(logger, &dest).context("fetching LLVM sources")?;

    Ok(0)
}

fn command_fetch_secure(logger: &Logger, args: &ArgMatches) -> Result<i32> {
    let url = args.value_of("url").expect("url argument is required");
    let sha256 = args
        .value_of("sha256")
        .expect("sha256 argument is required");
    let dest = args.value_of("dest").expect("dest argument is required");

    let dest = PathBuf::from(dest);

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("creating parent directory")?;
    }

    let name = dest
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let content = tugger_common::http::RemoteContent {
        name,
        url: url.to_string(),
        sha256: sha256.to_string(),
    };

    tugger_common::http::download_to_path(logger, &content, &dest)
        .context("downloading remote content")?;

    Ok(0)
}

fn command_fetch_support(logger: &Logger, args: &ArgMatches) -> Result<i32> {
    let dest = args.value_of("dest").expect("dest argument is required");

    let dest = PathBuf::from(dest);

    crate::downloads::fetch_linux_x86_64_support(logger, &dest)
        .context("fetching support artifacts")?;

    Ok(0)
}
