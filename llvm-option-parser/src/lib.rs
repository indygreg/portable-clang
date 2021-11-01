// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*! LLVM command option parsing.

This crate provides a mechanism for parsing arguments to LLVM programs.

It does so by consuming the LLVM tablegen data defining command options
and reimplementing an argument parser that takes this rich metadata into
account. This (hopefully) enables argument parsing to abide by the same
semantics. This includes the ability to recognize aliases and properly
recognize variations on argument parsing. e.g. `-I<value>` and `-I <value>`
being semantically equivalent.

Tablegen JSON data for LLVM commands is embedded in the crate and is
always available at run-time. This means you simply need a build of the
crate to parse LLVM command arguments.

# Higher-Level API

The API provided is currently rather low-level. We desire to implement a
lower-level API someday. For example, we want to turn clang's parsed options
into structs that convey the meaning of each invocation, such as whether we're
invoking a compiler, linker, etc.
 */

mod llvm;
pub use llvm::*;

use {once_cell::sync::Lazy, std::collections::BTreeMap, thiserror::Error};

const CLANG_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/clang.json");
const DSYMUTIL_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/dsymutil.json");
const LLD_COFF_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-coff.json");
const LLD_DARWIN_LD_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-darwin-ld.json");
const LLD_ELF_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-elf.json");
const LLD_MACHO_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-macho.json");
const LLD_MINGW_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-mingw.json");
const LLD_WASM_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/lld-wasm.json");
const LLVM_CVTRES_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-cvtres.json");
const LLVM_CXXFILT_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-cxxfilt.json");
const LLVM_DLLTOOL_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-dlltool.json");
const LLVM_LIB_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-lib.json");
const LLVM_ML_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-ml.json");
const LLVM_MT_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-mt.json");
const LLVM_NM_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-nm.json");
const LLVM_RC_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-rc.json");
const LLVM_READOBJ_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-readobj.json");
const LLVM_SIZE_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-size.json");
const LLVM_STRINGS_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-strings.json");
const LLVM_SYMBOLIZER_13_JSON: &[u8] = include_bytes!("tablegen/llvm-13/llvm-symbolizer.json");

/// Raw tablegen JSON for commands in LLVM version 13.
pub static LLVM_13_JSON: Lazy<BTreeMap<&str, &[u8]>> = Lazy::new(|| {
    BTreeMap::from_iter([
        ("clang", CLANG_13_JSON),
        ("dsymutil", DSYMUTIL_13_JSON),
        ("lld-coff", LLD_COFF_13_JSON),
        ("lld-darwin-ld", LLD_DARWIN_LD_13_JSON),
        ("lld-elf", LLD_ELF_13_JSON),
        ("lld-macho", LLD_MACHO_13_JSON),
        ("lld-mingw", LLD_MINGW_13_JSON),
        ("lld-wasm", LLD_WASM_13_JSON),
        ("llvm-cvtres", LLVM_CVTRES_13_JSON),
        ("llvm-cxxfilt", LLVM_CXXFILT_13_JSON),
        ("llvm-dlltool", LLVM_DLLTOOL_13_JSON),
        ("llvm-lib", LLVM_LIB_13_JSON),
        ("llvm-ml", LLVM_ML_13_JSON),
        ("llvm-mt", LLVM_MT_13_JSON),
        ("llvm-nm", LLVM_NM_13_JSON),
        ("llvm-cxxfilt", LLVM_CXXFILT_13_JSON),
        ("llvm-rc", LLVM_RC_13_JSON),
        ("llvm-readobj", LLVM_READOBJ_13_JSON),
        ("llvm-size", LLVM_SIZE_13_JSON),
        ("llvm-strings", LLVM_STRINGS_13_JSON),
        ("llvm-symbolizer", LLVM_SYMBOLIZER_13_JSON),
    ])
});

#[derive(Debug, Error)]
pub enum Error {
    #[error("unrecognized argument prefix: {0}")]
    UnrecognizedArgumentPrefix(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("JSON parsing error: {0}")]
    JsonParse(String),

    #[error("argument {0} missing required value")]
    ParseNoArgumentValue(String),

    #[error("argument {0} expected {1} values but only got {2}")]
    ParseMultipleValuesMissing(String, usize, usize),

    #[error("failed to resolve option alias {0} to {1}")]
    AliasMissing(String, String),
}

/// Obtain [CommandOptions] for a named command in LLVM version 13.
///
/// Tablegen JSON data for LLVM commands is embedded in the crate and
/// available to be parsed at run-time. Calling this function will trigger
/// the parsing of this data for the given command.
pub fn llvm_13_options(command: &str) -> Option<CommandOptions> {
    if let Some(data) = LLVM_13_JSON.get(command) {
        let cursor = std::io::Cursor::new(data);

        let options =
            CommandOptions::from_json(cursor).expect("built-in JSON should parse successfully");

        Some(options)
    } else {
        None
    }
}

/// Obtain LLVM option definitions for Clang version 13.
pub fn clang_13_options() -> CommandOptions {
    llvm_13_options("clang").expect("clang options should be available")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_all() {
        for command in LLVM_13_JSON.keys() {
            let options = llvm_13_options(command).unwrap();
            options.options_by_group();
            options.options_by_flag();
        }
    }

    #[test]
    fn clang_13() -> Result<(), Error> {
        let options = clang_13_options();

        assert_eq!(options.options[0].option_name, "C");
        assert_eq!(options.options[1].option_name, "CC");
        assert_eq!(options.options.last().unwrap().option_name, "y");

        Ok(())
    }

    #[test]
    fn parse_arg_flavors() -> Result<(), Error> {
        let options = clang_13_options();

        // Positional argument.
        let args = options.parse_arguments(vec!["clang"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0], ParsedArgument::Positional("clang".into()));

        // A flag argument with a single dash.
        let args = options.parse_arguments(vec!["-pthread"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("pthread"));

        // Joined with a single dash.
        let args = options.parse_arguments(vec!["-Wno-unused-result"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("W_Joined"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["no-unused-result"]);

        // Joined with equals value.
        let args = options.parse_arguments(vec!["-fvisibility=hidden"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("fvisibility_EQ"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["hidden"]);

        // Joined or separate joined flavor.
        let args = options.parse_arguments(vec!["-DDEBUG"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("D"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["DEBUG"]);

        // Joined or separate separate flavor.
        let args = options.parse_arguments(vec!["-D", "DEBUG"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("D"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["DEBUG"]);

        // Separate.
        let args = options.parse_arguments(vec!["-target", "value"])?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("target_legacy_spelling"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["value"]);
        // -target is an alias. Check that it resolves.
        let args = args.resolve_aliases(&options)?;
        assert_eq!(args.parsed.len(), 1);
        assert_eq!(args.parsed[0].name(), Some("target"));
        assert!(matches!(args.parsed[0], ParsedArgument::SingleValue(_, _)));
        assert_eq!(args.parsed[0].values(), vec!["value"]);

        Ok(())
    }
}
