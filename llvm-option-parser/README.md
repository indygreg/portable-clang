# llvm-option-parser

This crate provides an implementation of option parsing for LLVM commands.

It remodels LLVM's tablegen-based definitions of command options (it can
parse the output of `llvm-tblgen --dump-json`) so that it can nominally
parse command line arguments using the same semantics as LLVM commands
themselves.

The crate ships with JSON tablegen data for some LLVM commands, enabling
you to parse command line arguments for LLVM programs like `clang`.
