//! The commands that flush the DNS cache of each host, in the order that the
//! tool tries them.
//!
//! This module makes no system call, so its tests run on every host.

/// One way to flush a cache: the name of the cache, and the commands that
/// flush it, each run in turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub cache: &'static str,
    pub commands: Vec<Vec<&'static str>>,
}

/// macOS keeps two caches: the one of Directory Services, and the one of
/// mDNSResponder, which a hang-up signal empties.
pub fn macos() -> Method {
    Method {
        cache: "mDNSResponder",
        commands: vec![
            vec!["dscacheutil", "-flushcache"],
            vec!["killall", "-HUP", "mDNSResponder"],
        ],
    }
}

/// Windows keeps the cache of the DNS Client service.
pub fn windows() -> Method {
    Method {
        cache: "the DNS Client service",
        commands: vec![vec!["ipconfig", "/flushdns"]],
    }
}

/// Linux can keep a cache in systemd-resolved or in nscd, or none. Each
/// method whose program is on the host is a candidate, in this order.
pub fn linux(on_path: impl Fn(&str) -> bool) -> Vec<Method> {
    let mut out = Vec::new();
    if on_path("resolvectl") {
        out.push(Method {
            cache: "systemd-resolved",
            commands: vec![vec!["resolvectl", "flush-caches"]],
        });
    } else if on_path("systemd-resolve") {
        out.push(Method {
            cache: "systemd-resolved",
            commands: vec![vec!["systemd-resolve", "--flush-caches"]],
        });
    }
    if on_path("nscd") {
        out.push(Method {
            cache: "nscd",
            commands: vec![vec!["nscd", "-i", "hosts"]],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_tries_resolved_then_nscd() {
        let all = linux(|_| true);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].commands[0], ["resolvectl", "flush-caches"]);
        assert_eq!(all[1].cache, "nscd");
    }

    #[test]
    fn an_old_systemd_uses_its_old_command() {
        let old = linux(|p| p == "systemd-resolve");
        assert_eq!(old[0].commands[0], ["systemd-resolve", "--flush-caches"]);
    }

    #[test]
    fn a_host_with_no_cache_has_no_method() {
        assert!(linux(|_| false).is_empty());
    }

    #[test]
    fn macos_resets_both_caches() {
        assert_eq!(macos().commands.len(), 2);
    }
}
