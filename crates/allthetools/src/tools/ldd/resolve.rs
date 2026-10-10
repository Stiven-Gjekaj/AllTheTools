//! Find the file of each library, by the rules of the loader of its format.
//!
//! The search asks a [`Files`] whether a file exists, and never the disk
//! itself, so a test gives it a set of files in memory.
//!
//! This module makes no system call, so its tests run on every host.

use std::path::{Path, PathBuf};

use super::parse::{self, Binary, Kind};

/// The files that the search can see.
pub trait Files {
    fn is_file(&self, path: &Path) -> bool;
    /// The first `len` bytes of a file, or `None` when it cannot be read.
    fn head(&self, path: &Path, len: usize) -> Option<Vec<u8>>;
}

/// Where a library is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The library is this file.
    File(PathBuf),
    /// macOS keeps its own libraries in the dyld shared cache, and not as
    /// files. The name is the whole answer.
    SharedCache,
    /// Windows maps an API set name to a real library when the program
    /// starts. No file has this name.
    ApiSet,
    NotFound,
}

/// What the search knows about the host.
#[derive(Debug, Clone, Default)]
pub struct Host {
    /// The directories in LD_LIBRARY_PATH.
    pub ld_library_path: Vec<String>,
    /// The directories of /etc/ld.so.conf, then the default directories.
    pub elf_dirs: Vec<String>,
    /// The directories that Windows searches for a 64-bit program, after the
    /// directory of the program: System32, the Windows directory, then PATH.
    pub windows_dirs_64: Vec<PathBuf>,
    /// The same for a 32-bit program, with SysWOW64 in place of System32.
    pub windows_dirs_32: Vec<PathBuf>,
}

/// The binary that asks for a library, and the program that started the
/// chain.
pub struct Loader<'a> {
    pub binary: &'a Binary,
    pub path: &'a Path,
    pub main: &'a Binary,
    pub main_path: &'a Path,
}

pub fn find(name: &str, loader: &Loader<'_>, host: &Host, files: &dyn Files) -> Found {
    match loader.binary.kind {
        Kind::Elf { .. } => elf(name, loader, host, files),
        Kind::MachO { .. } => mach(name, loader, files),
        Kind::Pe { is_64, .. } => pe(name, loader, host, files, is_64),
    }
}

fn dir_of(path: &Path) -> String {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
        _ => ".".to_owned(),
    }
}

fn join(dir: &str, name: &str) -> PathBuf {
    PathBuf::from(format!("{}/{}", dir.trim_end_matches('/'), name))
}

/// The order of ld.so: DT_RPATH when the binary has no DT_RUNPATH, then
/// LD_LIBRARY_PATH, then DT_RUNPATH, then the cache and the defaults.
fn elf(name: &str, loader: &Loader<'_>, host: &Host, files: &dyn Files) -> Found {
    let binary = loader.binary;
    if name.contains('/') {
        let path = PathBuf::from(name);
        return if files.is_file(&path) {
            Found::File(path)
        } else {
            Found::NotFound
        };
    }

    let origin = dir_of(loader.path);
    let lib = match binary.kind {
        Kind::Elf { is_64: true, .. } => "lib64",
        _ => "lib",
    };
    let expand = |dir: &str| {
        dir.replace("${ORIGIN}", &origin)
            .replace("$ORIGIN", &origin)
            .replace("${LIB}", lib)
            .replace("$LIB", lib)
    };

    let mut dirs: Vec<String> = Vec::new();
    if binary.runpath.is_empty() {
        dirs.extend(binary.rpath.iter().map(|d| expand(d)));
        if !std::ptr::eq(loader.binary, loader.main) && loader.main.runpath.is_empty() {
            let main_origin = dir_of(loader.main_path);
            dirs.extend(loader.main.rpath.iter().map(|d| {
                d.replace("${ORIGIN}", &main_origin)
                    .replace("$ORIGIN", &main_origin)
            }));
        }
    }
    dirs.extend(host.ld_library_path.iter().cloned());
    dirs.extend(binary.runpath.iter().map(|d| expand(d)));
    dirs.extend(host.elf_dirs.iter().cloned());

    for dir in dirs {
        let path = join(&dir, name);
        if !files.is_file(&path) {
            continue;
        }
        let kind = files.head(&path, 20).and_then(|h| parse::elf_kind(&h));
        if kind == Some(binary.kind) {
            return Found::File(path);
        }
    }
    Found::NotFound
}

/// The rules of dyld: @rpath, @loader_path and @executable_path, then an
/// absolute path. A system library that is not a file is in the shared
/// cache.
fn mach(name: &str, loader: &Loader<'_>, files: &dyn Files) -> Found {
    let loader_dir = dir_of(loader.path);
    let exec_dir = dir_of(loader.main_path);
    let expand = |path: &str| {
        if let Some(rest) = path.strip_prefix("@loader_path") {
            format!("{loader_dir}{rest}")
        } else if let Some(rest) = path.strip_prefix("@executable_path") {
            format!("{exec_dir}{rest}")
        } else {
            path.to_owned()
        }
    };

    if let Some(rest) = name.strip_prefix("@rpath/") {
        let rpaths = loader.binary.rpath.iter().chain(&loader.main.rpath);
        for rpath in rpaths {
            let path = join(&expand(rpath), rest);
            if files.is_file(&path) {
                return Found::File(path);
            }
        }
        return Found::NotFound;
    }

    let path = PathBuf::from(expand(name));
    if files.is_file(&path) {
        Found::File(path)
    } else if name.starts_with("/usr/lib/") || name.starts_with("/System/Library/") {
        Found::SharedCache
    } else {
        Found::NotFound
    }
}

/// The search of Windows for a program that is not packaged: an API set,
/// then the directory of the program, then the system directories and PATH.
fn pe(name: &str, loader: &Loader<'_>, host: &Host, files: &dyn Files, is_64: bool) -> Found {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("api-ms-win-") || lower.starts_with("ext-ms-") {
        return Found::ApiSet;
    }
    let own = loader
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let system = if is_64 {
        &host.windows_dirs_64
    } else {
        &host.windows_dirs_32
    };
    for dir in std::iter::once(&own).chain(system) {
        let path = dir.join(name);
        if files.is_file(&path) {
            return Found::File(path);
        }
    }
    Found::NotFound
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Memory(HashMap<PathBuf, Vec<u8>>);

    impl Files for Memory {
        fn is_file(&self, path: &Path) -> bool {
            self.0.contains_key(path)
        }
        fn head(&self, path: &Path, len: usize) -> Option<Vec<u8>> {
            self.0.get(path).map(|b| b[..len.min(b.len())].to_vec())
        }
    }

    fn elf_head(is_64: bool, machine: u16) -> Vec<u8> {
        let mut head = vec![0u8; 64];
        head[..4].copy_from_slice(b"\x7fELF");
        head[4] = if is_64 { 2 } else { 1 };
        head[5] = 1;
        head[18..20].copy_from_slice(&machine.to_le_bytes());
        head
    }

    fn binary(kind: Kind) -> Binary {
        Binary {
            kind,
            needed: Vec::new(),
            rpath: Vec::new(),
            runpath: Vec::new(),
            interpreter: None,
        }
    }

    const X86_64: Kind = Kind::Elf {
        is_64: true,
        machine: 0x3e,
    };

    fn loader<'a>(binary: &'a Binary, path: &'a Path) -> Loader<'a> {
        Loader {
            binary,
            path,
            main: binary,
            main_path: path,
        }
    }

    #[test]
    fn elf_skips_a_library_of_the_wrong_class() {
        let files = Memory(HashMap::from([
            (PathBuf::from("/usr/lib/libz.so.1"), elf_head(false, 3)),
            (PathBuf::from("/usr/lib64/libz.so.1"), elf_head(true, 0x3e)),
        ]));
        let host = Host {
            elf_dirs: vec!["/usr/lib".into(), "/usr/lib64".into()],
            ..Host::default()
        };
        let bin = binary(X86_64);
        let path = PathBuf::from("/usr/bin/tool");
        assert_eq!(
            find("libz.so.1", &loader(&bin, &path), &host, &files),
            Found::File(PathBuf::from("/usr/lib64/libz.so.1"))
        );
    }

    #[test]
    fn elf_expands_origin_in_the_runpath() {
        let files = Memory(HashMap::from([(
            PathBuf::from("/opt/app/bin/../lib/libapp.so"),
            elf_head(true, 0x3e),
        )]));
        let mut bin = binary(X86_64);
        bin.runpath = vec!["$ORIGIN/../lib".into()];
        let path = PathBuf::from("/opt/app/bin/app");
        assert_eq!(
            find("libapp.so", &loader(&bin, &path), &Host::default(), &files),
            Found::File(PathBuf::from("/opt/app/bin/../lib/libapp.so"))
        );
    }

    #[test]
    fn elf_reads_ld_library_path_before_runpath_and_after_rpath() {
        let files = Memory(HashMap::from([
            (PathBuf::from("/rpath/liba.so"), elf_head(true, 0x3e)),
            (PathBuf::from("/env/liba.so"), elf_head(true, 0x3e)),
            (PathBuf::from("/runpath/liba.so"), elf_head(true, 0x3e)),
        ]));
        let host = Host {
            ld_library_path: vec!["/env".into()],
            ..Host::default()
        };
        let path = PathBuf::from("/bin/tool");

        let mut with_rpath = binary(X86_64);
        with_rpath.rpath = vec!["/rpath".into()];
        assert_eq!(
            find("liba.so", &loader(&with_rpath, &path), &host, &files),
            Found::File(PathBuf::from("/rpath/liba.so"))
        );

        // DT_RUNPATH turns DT_RPATH off, so the environment comes first.
        let mut both = with_rpath.clone();
        both.runpath = vec!["/runpath".into()];
        assert_eq!(
            find("liba.so", &loader(&both, &path), &host, &files),
            Found::File(PathBuf::from("/env/liba.so"))
        );
    }

    #[test]
    fn elf_without_the_file_is_not_found() {
        let files = Memory(HashMap::new());
        let bin = binary(X86_64);
        let path = PathBuf::from("/bin/tool");
        let host = Host {
            elf_dirs: vec!["/lib".into()],
            ..Host::default()
        };
        assert_eq!(
            find("libnone.so", &loader(&bin, &path), &host, &files),
            Found::NotFound
        );
    }

    const ARM64: Kind = Kind::MachO {
        cputype: 0x0100_000c,
    };

    #[test]
    fn mach_o_reads_each_rpath_with_the_loader_path() {
        let files = Memory(HashMap::from([(
            PathBuf::from("/Applications/App.app/Contents/MacOS/../Frameworks/Lib.dylib"),
            Vec::new(),
        )]));
        let mut bin = binary(ARM64);
        bin.rpath = vec!["/nowhere".into(), "@loader_path/../Frameworks".into()];
        let path = PathBuf::from("/Applications/App.app/Contents/MacOS/App");
        assert_eq!(
            find(
                "@rpath/Lib.dylib",
                &loader(&bin, &path),
                &Host::default(),
                &files
            ),
            Found::File(PathBuf::from(
                "/Applications/App.app/Contents/MacOS/../Frameworks/Lib.dylib"
            ))
        );
    }

    #[test]
    fn mach_o_system_library_is_in_the_shared_cache() {
        let files = Memory(HashMap::new());
        let bin = binary(ARM64);
        let path = PathBuf::from("/usr/local/bin/tool");
        let found = |name| find(name, &loader(&bin, &path), &Host::default(), &files);
        assert_eq!(found("/usr/lib/libSystem.B.dylib"), Found::SharedCache);
        assert_eq!(
            found("/System/Library/Frameworks/IOKit.framework/Versions/A/IOKit"),
            Found::SharedCache
        );
        assert_eq!(found("/opt/homebrew/lib/libnone.dylib"), Found::NotFound);
        assert_eq!(found("@rpath/libnone.dylib"), Found::NotFound);
    }

    #[test]
    fn pe_takes_the_directory_of_the_program_first() {
        let program_dir = PathBuf::from("C:/Program Files/App");
        let system = PathBuf::from("C:/Windows/System32");
        let files = Memory(HashMap::from([
            (program_dir.join("zlib1.dll"), Vec::new()),
            (system.join("zlib1.dll"), Vec::new()),
            (system.join("kernel32.dll"), Vec::new()),
        ]));
        let host = Host {
            windows_dirs_64: vec![system.clone()],
            ..Host::default()
        };
        let bin = binary(Kind::Pe {
            is_64: true,
            machine: 0x8664,
        });
        let path = program_dir.join("app.exe");
        let found = |name| find(name, &loader(&bin, &path), &host, &files);
        assert_eq!(
            found("zlib1.dll"),
            Found::File(program_dir.join("zlib1.dll"))
        );
        assert_eq!(
            found("kernel32.dll"),
            Found::File(system.join("kernel32.dll"))
        );
        assert_eq!(found("api-ms-win-crt-runtime-l1-1-0.dll"), Found::ApiSet);
        assert_eq!(found("missing.dll"), Found::NotFound);
    }

    #[test]
    fn pe_of_32_bits_uses_its_own_directories() {
        let wow = PathBuf::from("C:/Windows/SysWOW64");
        let files = Memory(HashMap::from([(wow.join("user32.dll"), Vec::new())]));
        let host = Host {
            windows_dirs_32: vec![wow.clone()],
            windows_dirs_64: vec![PathBuf::from("C:/Windows/System32")],
            ..Host::default()
        };
        let bin = binary(Kind::Pe {
            is_64: false,
            machine: 0x14c,
        });
        let path = PathBuf::from("C:/app/app.exe");
        assert_eq!(
            find("user32.dll", &loader(&bin, &path), &host, &files),
            Found::File(wow.join("user32.dll"))
        );
    }
}
