//! The Policy: which hosts are gated (docs/spec.md section 4).

use access::{Hostname, Policy};

fn host(name: &str) -> Hostname {
    Hostname::parse(name).unwrap()
}

#[test]
fn selected_mode_gates_only_listed_hosts() {
    let policy = Policy::Selected { hosts: [host("jellyfin.example.com")].into() };

    assert!(policy.is_gated(&host("jellyfin.example.com")));
    assert!(!policy.is_gated(&host("photos.example.com")));
    assert!(!policy.is_gated(&host("new-app.example.com")));
}

#[test]
fn all_mode_gates_everything_but_exclusions() {
    let policy = Policy::All { excluded: [host("status.example.com")].into() };

    assert!(policy.is_gated(&host("jellyfin.example.com")));
    assert!(policy.is_gated(&host("new-app.example.com")));
    assert!(!policy.is_gated(&host("status.example.com")));
}

#[test]
fn an_empty_selection_gates_nothing_and_says_so() {
    let empty = Policy::Selected { hosts: [].into() };
    let some = Policy::Selected { hosts: [host("jellyfin.example.com")].into() };

    assert!(!empty.is_gated(&host("jellyfin.example.com")));
    assert!(empty.gates_nothing());
    assert!(!some.gates_nothing());
    assert!(!Policy::All { excluded: [].into() }.gates_nothing());
}

#[test]
fn listed_hosts_are_the_selection_or_the_exclusions() {
    let policy = Policy::All { excluded: [host("b.example.com"), host("a.example.com")].into() };
    let listed: Vec<_> = policy.listed().map(Hostname::as_str).collect();

    assert_eq!(listed, ["a.example.com", "b.example.com"]);
}
