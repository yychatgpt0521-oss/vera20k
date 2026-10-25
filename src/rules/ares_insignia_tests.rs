//! Ares Insignia keys through the production Rules reader.
use crate::rules::ini_parser::IniFile;
use crate::rules::native_processing::RulesLayerStack;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::veterancy::VeterancyRank;

#[test]
fn insignia_base_key_fills_ranks_that_specific_keys_leave_unset() {
    // Mental Omega 3.3.6 heroes and [General] EnemyInsignia=no.
    let rules = RuleSet::from_rules_layers(&RulesLayerStack::new(IniFile::from_str(
        "[General]\nEnemyInsignia=no\n[VehicleTypes]\n0=HERO\n1=PLAIN\n\
         [HERO]\nInsignia=herostcr\nInsignia.Elite=herostar\nInsignia.ShowEnemy=yes\n",
    )))
    .unwrap();
    let hero = rules.object("HERO").unwrap();
    assert_eq!(hero.insignia(VeterancyRank::Rookie), Some("herostcr"));
    assert_eq!(hero.insignia(VeterancyRank::Veteran), Some("herostcr"));
    assert_eq!(hero.insignia(VeterancyRank::Elite), Some("herostar"));
    assert_eq!(hero.insignia_show_enemy(), Some(true));
    let plain = rules.object("PLAIN").unwrap();
    assert_eq!(plain.insignia(VeterancyRank::Elite), None);
    assert_eq!(plain.insignia_show_enemy(), None);
    assert!(!rules.general.enemy_insignia);
}
