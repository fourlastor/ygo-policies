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
    // Batteryman Fuel Cell.  Its cost can take away the only card it could
    // target: Tributing the monster an Equip Spell of the opponent's is
    // equipped to destroys the Spell.  The effect then goes on the chain with
    // no target, and CardScripts' operation fails on the one it does not
    // have (line 57).  EDOPro logs that error and the duel goes on, the
    // effect doing nothing; a duel here stopped.  Mended: it does nothing.
    // Found by all71 on 2026-10-20 (seed 47410671, Mark of the Rose on
    // Batteryman Charger; `tests/fixtures/batteryman-fuel-cell-without-a-target.json`).
    ("c74730899.lua", include_str!("../scripts/c74730899.lua")),
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The lines each fix mends: the line's number, CardScripts' line and
    /// the line played.
    const MENDED: &[(&str, &[(usize, &str, &str)])] = &[(
        "c74730899.lua",
        &[(57, "\tif tc:IsRelateToEffect(e) then", "\tif tc and tc:IsRelateToEffect(e) then")],
    )];

    #[test]
    fn a_fix_is_its_card_scripts_script_with_the_lines_it_mends() {
        let official = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/CardScripts/official");
        assert_eq!(super::SCRIPT_FIXES.len(), MENDED.len(), "every fix says what it mends");
        for ((name, fixed), (mended_name, mended)) in super::SCRIPT_FIXES.iter().zip(MENDED) {
            assert_eq!(name, mended_name);
            let upstream = std::fs::read_to_string(official.join(name)).unwrap();
            assert_eq!(upstream.lines().count(), fixed.lines().count(), "{name}");
            let differ: Vec<(usize, &str, &str)> = upstream
                .lines()
                .zip(fixed.lines())
                .enumerate()
                .filter(|(_, (before, after))| before != after)
                .map(|(index, (before, after))| (index + 1, before, after))
                .collect();
            assert_eq!(differ, mended.to_vec(), "{name}");
        }
    }
}
