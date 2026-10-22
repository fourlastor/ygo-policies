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
    // equipped to destroys the Spell.  The effect then went on the chain with
    // no target, and CardScripts' operation failed on the one it did not
    // have (line 57).  EDOPro logs that error and the duel goes on, the
    // effect doing nothing; a duel here stopped.  Found by all71 on
    // 2026-10-20 (seed 47410671, Mark of the Rose on Batteryman Charger;
    // `tests/fixtures/batteryman-fuel-cell-without-a-target.json`).  Played
    // as CardScripts mended it on 2026-10-22 (`188c4b93`,
    // ProjectIgnis/CardScripts#1281): the cost Tributes only a Batteryman
    // that leaves a card to target, so the effect is not offered where none
    // would be left.  Once the pin of `vendor/CardScripts` holds that
    // revision, the test below fails and this goes.
    ("c74730899.lua", include_str!("../scripts/c74730899.lua")),
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The lines each fix mends, a run of them at a time: the number of the
    /// run's first line, CardScripts' lines and the lines played in their
    /// place.
    const MENDED: &[(&str, &[(usize, &[&str], &[&str])])] = &[(
        "c74730899.lua",
        &[
            (
                36,
                &[
                    "function s.retcfilter(c)",
                    "\treturn c:IsSetCard(SET_BATTERYMAN)",
                    "end",
                    "function s.retfilter(c,e)",
                    "\treturn c:IsAbleToHand() and c:IsCanBeEffectTarget(e)",
                ],
                &[
                    "function s.retcfilter(c,tp)",
                    "\treturn c:IsSetCard(SET_BATTERYMAN) and Duel.IsExistingTarget(Card.IsAbleToHand,tp,0,LOCATION_ONFIELD,1,c:GetEquipGroup()+c)",
                ],
            ),
            (
                43,
                &[
                    "\tlocal dg=Duel.GetMatchingGroup(s.retfilter,tp,0,LOCATION_ONFIELD,nil,e)",
                    "\tif chk==0 then return Duel.CheckReleaseGroupCost(tp,s.retcfilter,1,false,aux.ReleaseCheckTarget,e:GetHandler(),dg) end",
                    "\tlocal g=Duel.SelectReleaseGroupCost(tp,s.retcfilter,1,1,false,aux.ReleaseCheckTarget,e:GetHandler(),dg)",
                ],
                &[
                    "\tif chk==0 then return Duel.CheckReleaseGroupCost(tp,s.retcfilter,1,false,nil,e:GetHandler(),tp) end",
                    "\tlocal g=Duel.SelectReleaseGroupCost(tp,s.retcfilter,1,1,false,nil,e:GetHandler(),tp)",
                ],
            ),
        ],
    )];

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
