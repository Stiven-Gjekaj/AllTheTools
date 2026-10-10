//! Read the libraries that a binary needs, from its own bytes.
//!
//! One reader takes the three formats: ELF from Linux, Mach-O from macOS and
//! PE from Windows. So `att ldd` reads a binary of any host on any host.
//!
//! This module makes no system call, so its tests run on every host.

use anyhow::{Result, bail};
use goblin::Object;
use goblin::mach::{Mach, SingleArch};

/// The format and the machine of a binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Elf { is_64: bool, machine: u16 },
    MachO { cputype: u32 },
    Pe { is_64: bool, machine: u16 },
}

/// What a binary says about the libraries that it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binary {
    pub kind: Kind,
    /// The names of the libraries, in the order that the binary gives them.
    pub needed: Vec<String>,
    /// The search paths of ELF DT_RPATH, or of Mach-O LC_RPATH.
    pub rpath: Vec<String>,
    /// The search paths of ELF DT_RUNPATH.
    pub runpath: Vec<String>,
    /// The program that loads an ELF binary, such as ld-linux.
    pub interpreter: Option<String>,
}

/// The CPU type of Mach-O for the host, so a universal binary gives the slice
/// that the host runs.
pub fn host_cputype() -> Option<u32> {
    if cfg!(target_arch = "aarch64") {
        Some(goblin::mach::cputype::CPU_TYPE_ARM64)
    } else if cfg!(target_arch = "x86_64") {
        Some(goblin::mach::cputype::CPU_TYPE_X86_64)
    } else {
        None
    }
}

/// Read a binary. A universal Mach-O binary gives the slice of `prefer`, or
/// the first slice when no slice matches.
pub fn read(bytes: &[u8], prefer: Option<u32>) -> Result<Binary> {
    let object = match Object::parse(bytes) {
        Ok(object) => object,
        Err(e) => bail!("this is not a binary that att ldd reads ({e})"),
    };
    match object {
        Object::Elf(elf) => Ok(Binary {
            kind: Kind::Elf {
                is_64: elf.is_64,
                machine: elf.header.e_machine,
            },
            needed: owned(&elf.libraries),
            rpath: split_paths(&elf.rpaths),
            runpath: split_paths(&elf.runpaths),
            interpreter: elf.interpreter.map(str::to_owned),
        }),
        Object::Mach(Mach::Binary(macho)) => Ok(mach(&macho)),
        Object::Mach(Mach::Fat(fat)) => {
            let mut first = None;
            for arch in fat.into_iter() {
                if let Ok(SingleArch::MachO(macho)) = arch {
                    let binary = mach(&macho);
                    if Some(macho.header.cputype) == prefer {
                        return Ok(binary);
                    }
                    first.get_or_insert(binary);
                }
            }
            match first {
                Some(binary) => Ok(binary),
                None => bail!("this universal binary holds no Mach-O slice"),
            }
        }
        Object::PE(pe) => Ok(Binary {
            kind: Kind::Pe {
                is_64: pe.is_64,
                machine: pe.header.coff_header.machine,
            },
            needed: owned(&pe.libraries),
            rpath: Vec::new(),
            runpath: Vec::new(),
            interpreter: None,
        }),
        Object::Archive(_) => {
            bail!("this is a static library, and it needs no library at run time")
        }
        _ => bail!("this is not an ELF, a Mach-O or a PE binary"),
    }
}

fn mach(macho: &goblin::mach::MachO<'_>) -> Binary {
    // goblin puts the name "self" first in the list, for the binary itself.
    let needed = macho
        .libs
        .iter()
        .filter(|name| **name != "self")
        .map(|name| (*name).to_owned())
        .collect();
    Binary {
        kind: Kind::MachO {
            cputype: macho.header.cputype,
        },
        needed,
        rpath: owned(&macho.rpaths),
        runpath: Vec::new(),
        interpreter: None,
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

/// An ELF search path can hold more than one directory, with a colon between
/// them.
fn split_paths(paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .flat_map(|p| p.split(':'))
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The class and the machine from the first bytes of an ELF file, or `None`
/// when the bytes are not ELF.
///
/// The loader of Linux skips a library of the wrong class or machine and
/// looks in the next directory, so the search does the same.
pub fn elf_kind(head: &[u8]) -> Option<Kind> {
    if head.len() < 20 || &head[..4] != b"\x7fELF" {
        return None;
    }
    let is_64 = head[4] == 2;
    let machine = if head[5] == 2 {
        u16::from_be_bytes([head[18], head[19]])
    } else {
        u16::from_le_bytes([head[18], head[19]])
    };
    Some(Kind::Elf { is_64, machine })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The test binary is a real binary of the host that runs the test, and the
    // compiler made it, so each host reads its own format here.
    fn this_test() -> Binary {
        let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        read(&bytes, host_cputype()).unwrap()
    }

    #[test]
    fn the_test_binary_needs_the_c_library_of_its_host() {
        let binary = this_test();
        let names = binary.needed.join(" ").to_ascii_lowercase();
        if cfg!(target_os = "linux") {
            assert!(matches!(binary.kind, Kind::Elf { .. }));
            assert!(names.contains("libc.so"), "{names}");
            assert!(binary.interpreter.is_some());
        } else if cfg!(target_os = "macos") {
            assert!(matches!(binary.kind, Kind::MachO { .. }));
            assert!(names.contains("/usr/lib/libsystem.b.dylib"), "{names}");
        } else if cfg!(windows) {
            assert!(matches!(binary.kind, Kind::Pe { is_64: true, .. }));
            assert!(names.contains("kernel32.dll"), "{names}");
        }
    }

    #[test]
    fn mach_o_does_not_name_itself() {
        assert!(!this_test().needed.iter().any(|n| n == "self"));
    }

    #[test]
    fn text_is_not_a_binary() {
        assert!(read(b"#!/bin/sh\necho hello\n", None).is_err());
        assert!(read(b"", None).is_err());
    }

    #[test]
    fn a_search_path_splits_at_each_colon() {
        assert_eq!(split_paths(&["/a:/b", "", "/c::"]), ["/a", "/b", "/c"]);
    }

    #[test]
    fn elf_kind_reads_the_class_and_the_machine() {
        let mut head = vec![0u8; 20];
        head[..4].copy_from_slice(b"\x7fELF");
        head[4] = 2;
        head[5] = 1;
        head[18] = 0x3e;
        assert_eq!(
            elf_kind(&head),
            Some(Kind::Elf {
                is_64: true,
                machine: 0x3e
            })
        );
        head[4] = 1;
        head[5] = 2;
        head[18] = 0;
        head[19] = 0x28;
        assert_eq!(
            elf_kind(&head),
            Some(Kind::Elf {
                is_64: false,
                machine: 0x28
            })
        );
        assert_eq!(elf_kind(b"MZ\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"), None);
        assert_eq!(elf_kind(b"\x7fELF"), None);
    }
}
