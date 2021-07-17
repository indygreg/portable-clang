// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    crate::build::Environment,
    anyhow::{anyhow, Context, Result},
    clap::{App, AppSettings, Arg, ArgMatches, SubCommand},
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
        SubCommand::with_name("build-clang")
            .about("Build Clang core artifact")
            .arg(
                Arg::with_name("bootstrap_dir")
                    .long("--bootstrap-dir")
                    .takes_value(true)
                    .help("Directory containing gcc toolchain artifact used to bootstrap clang"),
            )
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Destination directory to write artifacts to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("build-gcc")
            .about("Build GCC artifacts needed to bootstrap Clang")
            .arg(
                Arg::with_name("dest")
                    .required(true)
                    .help("Destination directory to write artifacts to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("docker-image-clang")
            .about("Build Docker image for building Clang")
            .arg(
                Arg::with_name("dest")
                    .long("--dest")
                    .takes_value(true)
                    .help("Destination file to write zstd compressed image to"),
            ),
    );

    let app = app.subcommand(
        SubCommand::with_name("docker-image-gcc")
            .about("Build Docker image for building GCC")
            .arg(
                Arg::with_name("dest")
                    .long("--dest")
                    .takes_value(true)
                    .help("Destination file to write zstd compressed image to"),
            ),
    );

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

    let env = Environment::new(logger)?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            match matches.subcommand() {
                ("build-clang", Some(args)) => command_build_clang(env, args).await,
                ("build-gcc", Some(args)) => command_build_gcc(env, args).await,
                ("docker-image-clang", Some(args)) => command_docker_image_clang(env, args).await,
                ("docker-image-gcc", Some(args)) => command_docker_image_gcc(env, args).await,
                ("fetch-gcc-sources", Some(args)) => command_fetch_gcc_sources(env, args).await,
                ("fetch-llvm-sources", Some(args)) => command_fetch_llvm_sources(env, args).await,
                ("fetch-secure", Some(args)) => command_fetch_secure(env, args).await,
                ("fetch-support", Some(args)) => command_fetch_support(env, args).await,
                _ => Err(anyhow!("invalid sub-command")),
            }
        })
}

async fn command_build_clang<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    let dest_dir = PathBuf::from(args.value_of_os("dest").expect("dest argument is required"));
    let bootstrap_dir = args.value_of_os("bootstrap_dir").map(PathBuf::from);

    env.build_clang(&dest_dir, bootstrap_dir).await?;

    Ok(0)
}

async fn command_build_gcc<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    let dest_dir = PathBuf::from(args.value_of_os("dest").expect("dest argument is required"));

    env.build_gcc(&dest_dir).await?;

    Ok(0)
}

async fn command_docker_image_clang<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    env.docker_image_clang(args.value_of_os("dest")).await?;

    Ok(0)
}

async fn command_docker_image_gcc<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    env.docker_image_gcc(args.value_of_os("dest")).await?;

    Ok(0)
}

async fn command_fetch_gcc_sources<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    let dest = PathBuf::from(args.value_of_os("dest").expect("dest argument is required"));

    crate::downloads::fetch_gcc_sources(env.logger(), &dest).context("fetching GCC sources")?;

    Ok(0)
}

async fn command_fetch_llvm_sources<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    let dest = PathBuf::from(args.value_of_os("dest").expect("dest argument is required"));

    crate::downloads::fetch_llvm_sources(env.logger(), &dest).context("fetching LLVM sources")?;

    Ok(0)
}

async fn command_fetch_secure<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
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

    tugger_common::http::download_to_path(env.logger(), &content, &dest)
        .context("downloading remote content")?;

    Ok(0)
}

async fn command_fetch_support<'a>(env: Environment, args: &ArgMatches<'a>) -> Result<i32> {
    let dest = PathBuf::from(args.value_of_os("dest").expect("dest argument is required"));

    crate::downloads::fetch_linux_x86_64_support(env.logger(), &dest)
        .context("fetching support artifacts")?;

    Ok(0)
}
