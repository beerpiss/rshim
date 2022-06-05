# `rshim`

rshim is the [`shim`](https://github.com/71/scoop-better-shimexe) program written in rust.

## Why does this exist?
There are several versions of `shim`:
- the official [`shim.cs`](https://github.com/lukesampson/scoop/blob/master/supporting/shimexe/shim.cs) was written in c# and required an instantiation of a .NET command line app every time it was started therefore is slower than directly executing. And it dose not handle Ctrl+C event correctly.

- [`shim.c`](https://github.com/71/scoop-better-shimexe) is ok with performance and Ctrl+C event handling,but sometimes suffer memory violation (randomly exited with return code `3221226356`).

- [`shim.cpp`](https://github.com/kiennq/scoop-better-shimexe) didn't work on my machine, calling it with any executable result in an infinite recursion of creating subprocess until eating all memory for unknown subtle reason.

### Advantages of `rshim`
- less probability with bug and undefined behavior.
- properly handle variety errors.
- friendly error message.
- properly handle Ctrl+C events.
- properly handle utf-8 with or without bom in shim file.

### Differences from upstream `rshim`
- relies heavily on Win32 API, moreso than the original
- reduced binary size massively (134KB static compared to 300KB)
- handle quoted strings
- some slight speed improvements

## Installation
First install [rust](https://rustup.rs/)
```shell
cargo build --release
```

Close any running process with existing shims, then
```shell
./repshims.bat
```

## Roadmap
- [ ] remove `core::fmt` entirely
- [ ] make it run with `#![no_std]`

## Executable Size
- Without `crt-static` enabled: around 48KB
- With `crt-static` enabled: around 134KB
