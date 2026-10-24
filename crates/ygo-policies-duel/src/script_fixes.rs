//! Card scripts played in place of those of `vendor/CardScripts`: each one is
//! that repository's script with a fault mended, kept under `scripts/` beside
//! this crate's sources.  CardScripts is ProjectIgnis's own repository, which
//! this one does not change; [`Core::open`](crate::Core::open) reads these
//! before it.
//!
//! The test below holds each to the script it mends, line for line but for
//! the lines it mends: a change of that script upstream fails it, and the
//! mend wants a look.

/// A script's file name, and the script played under it.
pub(crate) const SCRIPT_FIXES: &[(&str, &str)] = &[
    // None now. The last, Batteryman Fuel Cell's (a cost that could take away
    // the only card it could target), is CardScripts' own since its pin holds
    // `188c4b93`, ProjectIgnis/CardScripts#1281.
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The lines each fix mends, a run of them at a time: the number of the
    /// run's first line, CardScripts' lines and the lines played in their
    /// place.
    const MENDED: &[(&str, &[(usize, &[&str], &[&str])])] = &[];

    #[test]
    fn a_fix_is_its_card_scripts_script_with_the_lines_it_mends() {
        let official = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/CardScripts/official");
        assert_eq!(super::SCRIPT_FIXES.len(), MENDED.len(), "every fix says what it mends");
        for ((name, fixed), (mended_name, mended)) in super::SCRIPT_FIXES.iter().zip(MENDED) {
            assert_eq!(name, mended_name);
            let upstream = std::fs::read_to_string(official.join(name)).unwrap();
            let upstream: Vec<&str> = upstream.lines().collect();
            // CardScripts' script, each run of lines it mends played instead.
            let mut played = Vec::new();
            let mut next = 0;
            for (first, before, after) in mended.iter() {
                let start = first - 1;
                assert!(start >= next, "{name}: the runs are in order");
                played.extend_from_slice(&upstream[next..start]);
                assert_eq!(upstream.get(start..start + before.len()), Some(*before), "{name}, line {first}");
                played.extend_from_slice(after);
                next = start + before.len();
            }
            played.extend_from_slice(&upstream[next..]);
            assert_eq!(fixed.lines().collect::<Vec<_>>(), played, "{name}");
        }
    }
}
