//! `att ldd`: list the libraries that a binary needs.
//!
//! Linux has `ldd`. macOS has `otool -L`, which names only the libraries of
//! the first level and finds no file. Windows has `dumpbin`, which comes only
//! with Visual Studio. This tool reads ELF, Mach-O and PE on every host, and
//! finds each library by the rules of the loader of its format.
//!
//! It reads the binary and runs nothing. The `ldd` of Linux can run the
//! program, and this one cannot.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

pub mod parse;
pub mod resolve;

use parse::{Binary, Kind};
use resolve::{Files, Found, Host, Loader};

/// List the libraries that a binary needs, and where each one is.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The binaries to read: a program or a library.
    #[arg(required = true)]
    pub files: Vec<PathBuf>,

    /// Show each library under the binary that needs it.
    #[arg(long, conflicts_with = "direct")]
    pub tree: bool,

    /// Show only the libraries that the binary names itself.
    #[arg(long)]
    pub direct: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

/// One library in the result.
#[derive(Debug, Default, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: Option<PathBuf>,
    pub status: &'static str,
    /// 1 for a library that the binary names, 2 for a library of that
    /// library, and so on.
    pub depth: usize,
    /// In a tree, the library is already in the tree above this line.
    pub repeat: bool,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Report {
    pub(crate) file: PathBuf,
    pub(crate) format: String,
    pub(crate) interpreter: Option<String>,
    pub(crate) libraries: Vec<Entry>,
}

pub fn run(args: Args) -> Result<i32> {
    let host = host();
    let mut missing = 0;
    let mut failed = false;
    let many = args.files.len() > 1;

    for file in &args.files {
        let report = match report(file, &host, &args) {
            Ok(report) => report,
            Err(e) => {
                eprintln!("att ldd: {}: {e:#}", file.display());
                failed = true;
                continue;
            }
        };
        missing += report
            .libraries
            .iter()
            .filter(|e| e.status == "not found" && !e.repeat)
            .count();
        if args.json {
            println!("{}", serde_json::to_string(&report)?);
        } else {
            print(&report, many, args.tree);
        }
    }

    if missing > 0 {
        eprintln!("att ldd: {missing} libraries were not found");
    }
    Ok(if failed || missing > 0 { 1 } else { 0 })
}

fn report(file: &Path, host: &Host, args: &Args) -> Result<Report> {
    let bytes = std::fs::read(file).context("could not read the file")?;
    let binary = parse::read(&bytes, parse::host_cputype())?;
    let libraries = if args.direct {
        walk(file, &binary, host, &Disk, Some(1), false)
    } else {
        walk(file, &binary, host, &Disk, None, args.tree)
    };
    Ok(Report {
        file: file.to_path_buf(),
        format: format_name(binary.kind),
        interpreter: binary.interpreter.clone(),
        libraries,
    })
}

fn print(report: &Report, many: bool, tree: bool) {
    if many {
        println!("{}:", report.file.display());
    }
    if let Some(interpreter) = &report.interpreter {
        println!("\t{interpreter} (interpreter)");
    }
    for entry in &report.libraries {
        let indent = if tree {
            "    ".repeat(entry.depth - 1)
        } else {
            String::new()
        };
        let place = match (entry.status, &entry.path) {
            ("found", Some(path)) => format!(" => {}", path.display()),
            ("shared cache", _) => " (in the dyld shared cache)".to_owned(),
            ("API set", _) => " (API set, which Windows maps when the program starts)".to_owned(),
            _ => " => not found".to_owned(),
        };
        let seen = if entry.repeat { " (see above)" } else { "" };
        println!("\t{indent}{}{place}{seen}", entry.name);
    }
}

/// Walk the libraries. A flat walk goes level by level and names each
/// library one time. A tree walk goes depth first, and names a library again
/// with a mark when another binary needs it too.
pub fn walk(
    path: &Path,
    root: &Binary,
    host: &Host,
    files: &dyn Files,
    max_depth: Option<usize>,
    tree: bool,
) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if tree {
        visit(path, root, path, root, host, files, 1, &mut seen, &mut out);
        return out;
    }

    let mut queue = VecDeque::from([(path.to_path_buf(), root.clone(), 1usize)]);
    while let Some((from_path, from, depth)) = queue.pop_front() {
        if max_depth.is_some_and(|max| depth > max) {
            continue;
        }
        let loader = Loader {
            binary: &from,
            path: &from_path,
            main: root,
            main_path: path,
        };
        for name in &from.needed {
            if !seen.insert(key(name, root.kind)) {
                continue;
            }
            let found = resolve::find(name, &loader, host, files);
            if let Found::File(file) = &found
                && let Some(child) = read_library(file, root.kind)
            {
                queue.push_back((file.clone(), child, depth + 1));
            }
            out.push(entry(name, found, depth, false));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn visit(
    from_path: &Path,
    from: &Binary,
    main_path: &Path,
    main: &Binary,
    host: &Host,
    files: &dyn Files,
    depth: usize,
    seen: &mut HashSet<String>,
    out: &mut Vec<Entry>,
) {
    let loader = Loader {
        binary: from,
        path: from_path,
        main,
        main_path,
    };
    for name in &from.needed {
        let found = resolve::find(name, &loader, host, files);
        let first = seen.insert(key(name, main.kind));
        let child = match (&found, first) {
            (Found::File(file), true) => read_library(file, main.kind).map(|b| (file.clone(), b)),
            _ => None,
        };
        out.push(entry(name, found, depth, !first));
        if let Some((file, binary)) = child {
            visit(
                &file,
                &binary,
                main_path,
                main,
                host,
                files,
                depth + 1,
                seen,
                out,
            );
        }
    }
}

/// Windows names a library in any case, so the key of a PE library is in
/// small letters.
fn key(name: &str, kind: Kind) -> String {
    match kind {
        Kind::Pe { .. } => name.to_ascii_lowercase(),
        _ => name.to_owned(),
    }
}

fn read_library(path: &Path, kind: Kind) -> Option<Binary> {
    let bytes = std::fs::read(path).ok()?;
    let prefer = match kind {
        Kind::MachO { cputype } => Some(cputype),
        _ => None,
    };
    parse::read(&bytes, prefer).ok()
}

fn entry(name: &str, found: Found, depth: usize, repeat: bool) -> Entry {
    let (path, status) = match found {
        Found::File(path) => (Some(path), "found"),
        Found::SharedCache => (None, "shared cache"),
        Found::ApiSet => (None, "API set"),
        Found::NotFound => (None, "not found"),
    };
    Entry {
        name: name.to_owned(),
        path,
        status,
        depth,
        repeat,
    }
}

fn format_name(kind: Kind) -> String {
    match kind {
        Kind::Elf { is_64, .. } => format!("ELF {}-bit", if is_64 { 64 } else { 32 }),
        Kind::MachO { .. } => "Mach-O".to_owned(),
        Kind::Pe { is_64, .. } => format!("PE {}-bit", if is_64 { 64 } else { 32 }),
    }
}

/// The files on the disk of this computer.
struct Disk;

impl Files for Disk {
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn head(&self, path: &Path, len: usize) -> Option<Vec<u8>> {
        use std::io::Read;
        let mut head = vec![0u8; len];
        let mut file = std::fs::File::open(path).ok()?;
        let read = file.read(&mut head).ok()?;
        head.truncate(read);
        Some(head)
    }
}

/// What the search needs to know about this computer. Each host fills only
/// its own part. A binary of another host finds only the libraries next to
/// it.
fn host() -> Host {
    let mut host = Host::default();
    if cfg!(target_os = "linux") {
        if let Some(paths) = std::env::var_os("LD_LIBRARY_PATH") {
            host.ld_library_path = std::env::split_paths(&paths)
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
        }
        let conf = std::fs::read_to_string("/etc/ld.so.conf").unwrap_or_default();
        host.elf_dirs = parse::ld_so_conf(&conf, &mut include);
        for dir in ["/lib64", "/usr/lib64", "/lib", "/usr/lib"] {
            if !host.elf_dirs.iter().any(|d| d == dir) {
                host.elf_dirs.push(dir.to_owned());
            }
        }
    }
    if cfg!(windows) {
        let root =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()));
        let path: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        host.windows_dirs_64 = [root.join("System32"), root.clone()]
            .into_iter()
            .chain(path.iter().cloned())
            .collect();
        host.windows_dirs_32 = [root.join("SysWOW64"), root]
            .into_iter()
            .chain(path)
            .collect();
    }
    host
}

/// The texts of the files that an `include` line of ld.so.conf names. A
/// relative pattern is relative to /etc, and `*` stands for any part of a
/// file name.
fn include(pattern: &str) -> Vec<String> {
    let pattern = if pattern.starts_with('/') {
        pattern.to_owned()
    } else {
        format!("/etc/{pattern}")
    };
    let (dir, name) = pattern.rsplit_once('/').unwrap_or(("/etc", &pattern));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| wildcard(name, n))
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| std::fs::read_to_string(format!("{dir}/{n}")).ok())
        .collect()
}

/// True when `name` matches `pattern`, where `*` stands for any text.
fn wildcard(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || !name[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(at) => rest = &rest[at + middle.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::wildcard;

    #[test]
    fn wildcard_matches_like_a_shell() {
        assert!(wildcard("*.conf", "libc.conf"));
        assert!(wildcard("*.conf", ".conf"));
        assert!(!wildcard("*.conf", "libc.conf.bak"));
        assert!(wildcard("x86*gnu.conf", "x86_64-linux-gnu.conf"));
        assert!(wildcard("a*b*c", "aXbYc"));
        assert!(!wildcard("a*b*c", "aXcYb"));
        assert!(wildcard("exact", "exact"));
        assert!(!wildcard("ab*ba", "aba"));
    }

    // The test binary is a binary of the host, so the walk reaches real
    // libraries. Each one that the walk names is found, because the test
    // started and the loader of the host found them all.
    #[test]
    fn the_test_binary_needs_nothing_that_is_missing() {
        let path = std::env::current_exe().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let binary = super::parse::read(&bytes, super::parse::host_cputype()).unwrap();
        let entries = super::walk(&path, &binary, &super::host(), &super::Disk, None, false);
        assert!(!entries.is_empty());
        let missing: Vec<_> = entries.iter().filter(|e| e.status == "not found").collect();
        assert!(missing.is_empty(), "{missing:?}");
    }
}
