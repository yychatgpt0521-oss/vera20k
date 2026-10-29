//! Ares `[MouseCursors]`: the named mouse cursor table every cursor-valued
//! key resolves against (docs: new/mousecursors, new/cursors).
//!
//! Installed Ares 3.0 evidence: the table starts as the 86 rows of the native
//! cursor descriptor table (`0x0082D028`, 28 bytes a row) under the names at
//! `0x10094B20`, then the Ares defaults built by copying native rows
//! (`0x10007214..0x100074A8`). RulesClass_BeforeTypeData (`0x679A15` ->
//! `0x1002AEF0`) reads `[MouseCursors]` each Rules pass
//! (`0x1002B125..0x1002B236`): every key naming no cursor yet appends one
//! (constructor `0x100070D0`), then each cursor reads its own key
//! (`0x100075B0`). Type keys (`Cursor.Move=` and the others) store an index
//! into this table (`0x1000FB50`: case-insensitive name lookup, `none` or an
//! unknown name keeps the old index).

use crate::rules::ini_parser::IniFile;

/// One cursor descriptor: the native row's seven fields. Hotspots keep the
/// native sentinels (`0` near edge, `12345` centre, `54321` far edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MouseCursor {
    pub frame: i32,
    pub count: i32,
    /// Frame rate in cursor clock units (16 ms); 0 is a still frame.
    pub interval: i32,
    /// First radar frame; -1 draws the main set on the radar too.
    pub mini_frame: i32,
    pub mini_count: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
}

const CENTRE: i32 = 12345;
const FAR: i32 = 54321;

const fn row(
    frame: i32,
    count: i32,
    interval: i32,
    mini_frame: i32,
    mini_count: i32,
    hotspot_x: i32,
    hotspot_y: i32,
) -> MouseCursor {
    MouseCursor {
        frame,
        count,
        interval,
        mini_frame,
        mini_count,
        hotspot_x,
        hotspot_y,
    }
}

const fn mid(
    frame: i32,
    count: i32,
    interval: i32,
    mini_frame: i32,
    mini_count: i32,
) -> MouseCursor {
    row(
        frame, count, interval, mini_frame, mini_count, CENTRE, CENTRE,
    )
}

const fn edge(frame: i32, hotspot_x: i32, hotspot_y: i32) -> MouseCursor {
    row(frame, 1, 0, -1, -1, hotspot_x, hotspot_y)
}

/// The native descriptor rows (`0x0082D028`) under Ares's names
/// (`0x10094B20`), in row order.
pub const NATIVE_ROWS: [(&str, MouseCursor); 86] = [
    ("Default", row(0, 1, 0, 1, 1, 0, 0)),
    ("MoveN", edge(2, CENTRE, 0)),
    ("MoveNE", edge(3, FAR, 0)),
    ("MoveE", edge(4, FAR, CENTRE)),
    ("MoveSE", edge(5, FAR, FAR)),
    ("MoveS", edge(6, CENTRE, FAR)),
    ("MoveSW", edge(7, 0, FAR)),
    ("MoveW", edge(8, 0, CENTRE)),
    ("MoveNW", edge(9, 0, 0)),
    ("NoMoveN", edge(10, CENTRE, 0)),
    ("NoMoveNE", edge(11, FAR, 0)),
    ("NoMoveE", edge(12, FAR, CENTRE)),
    ("NoMoveSE", edge(13, FAR, FAR)),
    ("NoMoveS", edge(14, CENTRE, FAR)),
    ("NoMoveSW", edge(15, 0, FAR)),
    ("NoMoveW", edge(16, 0, CENTRE)),
    ("NoMoveNW", edge(17, 0, 0)),
    ("Select", mid(18, 13, 4, -1, -1)),
    ("Move", mid(31, 10, 4, 42, 10)),
    ("NoMove", mid(41, 1, 0, 52, 1)),
    ("Attack", mid(53, 5, 4, 63, 5)),
    ("AttackOutOfRange", mid(58, 5, 4, 63, 5)),
    ("CUR_16", mid(68, 5, 4, 73, 5)),
    ("DesolatorDeploy", mid(78, 10, 4, -1, 10)),
    ("CUR_18", mid(88, 1, 0, -1, 1)),
    ("Enter", mid(89, 10, 4, 100, 10)),
    ("NoEnter", mid(99, 1, 0, 63, 1)),
    ("Deploy", mid(110, 9, 4, -1, -1)),
    ("NoDeploy", mid(119, 1, 0, -1, -1)),
    ("CUR_1D", mid(120, 9, 4, -1, -1)),
    ("Sell", mid(129, 10, 4, -1, -1)),
    ("SellUnit", mid(139, 10, 4, -1, -1)),
    ("NoSell", mid(149, 1, 0, -1, -1)),
    ("Repair", mid(150, 20, 4, -1, -1)),
    ("EngineerRepair", mid(170, 20, 4, -1, -1)),
    ("NoRepair", mid(190, 1, 0, -1, -1)),
    ("CUR_24", mid(191, 7, 0, -1, -1)),
    ("Disguise", mid(199, 5, 0, -1, -1)),
    ("IvanBomb", mid(204, 5, 0, -1, -1)),
    ("MindControl", mid(209, 5, 0, -1, -1)),
    ("RemoveSquid", mid(214, 5, 0, -1, -1)),
    ("Crush", mid(219, 5, 0, -1, -1)),
    ("SpyTech", mid(224, 5, 0, -1, -1)),
    ("SpyPower", mid(229, 5, 0, -1, -1)),
    ("CUR_2C", mid(234, 5, 0, -1, -1)),
    ("GIDeploy", mid(239, 10, 0, -1, -1)),
    ("CUR_2E", mid(249, 10, 0, -1, -1)),
    ("Paradrop", mid(259, 10, 0, 516, -1)),
    ("CUR_30", mid(269, 10, 0, -1, -1)),
    ("CUR_31", mid(356, 1, 0, -1, -1)),
    ("LightningStorm", mid(279, 20, 4, 514, 1)),
    ("Detonate", mid(299, 10, 0, -1, -1)),
    ("Demolish", mid(309, 10, 4, -1, -1)),
    ("Nuke", mid(319, 10, 4, 513, 1)),
    ("CUR_36", mid(329, 10, 0, -1, -1)),
    ("Power", mid(339, 6, 0, -1, -1)),
    ("CUR_38", mid(345, 1, 0, -1, -1)),
    ("IronCurtain", mid(346, 5, 0, -1, -1)),
    ("Chronosphere", mid(357, 12, 0, -1, -1)),
    ("Disarm", mid(369, 15, 0, -1, -1)),
    ("CUR_3C", mid(384, 1, 0, -1, -1)),
    ("Scroll", mid(385, 1, 0, -1, -1)),
    ("ScrollESW", mid(386, 1, 0, -1, -1)),
    ("ScrollSW", mid(387, 1, 0, -1, -1)),
    ("ScrollNSW", mid(388, 1, 0, -1, -1)),
    ("ScrollNW", mid(389, 1, 0, -1, -1)),
    ("ScrollNEW", mid(390, 1, 0, -1, -1)),
    ("ScrollNE", mid(391, 1, 0, -1, -1)),
    ("ScrollNES", mid(392, 1, 0, -1, -1)),
    ("ScrollES", mid(393, 1, 0, -1, -1)),
    ("CUR_46", mid(394, 10, 4, -1, -1)),
    ("AttackMove", mid(404, 9, 4, 63, 5)),
    ("CUR_48", mid(413, 9, 4, -1, -1)),
    ("InfantryAbsorb", mid(422, 9, 4, -1, -1)),
    ("NoMindControl", mid(431, 1, 0, -1, -1)),
    ("CUR_4B", mid(432, 1, 0, -1, -1)),
    ("CUR_4C", mid(433, 1, 0, -1, -1)),
    ("CUR_4D", mid(434, 1, 0, -1, -1)),
    ("Beacon", mid(435, 15, 4, -1, -1)),
    ("ForceShield", mid(450, 10, 4, -1, 1)),
    ("NoForceShield", mid(460, 10, 4, -1, -1)),
    ("GeneticMutator", mid(470, 10, 4, -1, -1)),
    ("AirStrike", mid(480, 8, 4, -1, -1)),
    ("PsychicDominator", mid(488, 8, 4, 516, -1)),
    ("PsychicReveal", mid(496, 8, 4, 515, 1)),
    ("SpyPlane", mid(504, 8, 4, 512, 1)),
];

/// Where an Ares default cursor's fields come from.
enum AresDefault {
    /// A copy of this native row.
    Row(usize),
    Literal(MouseCursor),
}

/// The Ares defaults after the native rows, in order: each copies a native
/// row, except InfantryHeal's literal (`0x100072B4`).
const ARES_DEFAULTS: [(&str, AresDefault); 9] = [
    ("Tote", AresDefault::Row(54)),
    ("EngineerDamage", AresDefault::Row(51)),
    ("TogglePower", AresDefault::Row(55)),
    ("NoTogglePower", AresDefault::Row(60)),
    ("InfantryHeal", AresDefault::Literal(mid(355, 1, 0, -1, -1))),
    ("UnitRepair", AresDefault::Row(33)),
    ("TakeVehicle", AresDefault::Row(25)),
    ("Sabotage", AresDefault::Row(25)),
    ("RepairTrench", AresDefault::Row(33)),
];

/// Ares's index of its `EngineerDamage` cursor (the multi-engineer damage
/// action's, `0x10025565`).
pub const ENGINEER_DAMAGE: u16 = 87;
/// Ares's index of its `TakeVehicle` cursor (the hijack action's,
/// `0x100496EF`).
pub const TAKE_VEHICLE: u16 = 92;

/// A new cursor's constructor fields (`0x10007413..0x1000743D`).
const NEW_CURSOR: MouseCursor = row(0, 1, 0, -1, -1, 0, 0);

/// The retained Ares cursor table: native rows, Ares defaults, then the
/// names `[MouseCursors]` added, in that order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MouseCursorCatalog {
    entries: Vec<(String, MouseCursor)>,
}

impl Default for MouseCursorCatalog {
    fn default() -> Self {
        let mut entries: Vec<(String, MouseCursor)> = NATIVE_ROWS
            .iter()
            .map(|(name, cursor)| ((*name).to_owned(), *cursor))
            .collect();
        for (name, source) in ARES_DEFAULTS {
            let cursor = match source {
                AresDefault::Row(row) => NATIVE_ROWS[row].1,
                AresDefault::Literal(cursor) => cursor,
            };
            entries.push((name.to_owned(), cursor));
        }
        Self { entries }
    }
}

impl MouseCursorCatalog {
    /// The case-insensitive name lookup (`0x10007520`).
    pub fn index_of(&self, name: &str) -> Option<u16> {
        self.entries
            .iter()
            .position(|(entry, _)| entry.eq_ignore_ascii_case(name))
            .and_then(|index| u16::try_from(index).ok())
    }

    pub fn get(&self, index: u16) -> Option<MouseCursor> {
        self.entries
            .get(usize::from(index))
            .map(|(_, cursor)| *cursor)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A type key's cursor index (`0x1000FB50`); `None` keeps the type's
    /// default cursor.
    pub fn resolve(&self, name: Option<&str>) -> Option<u16> {
        self.index_of(name?)
    }

    /// One Rules pass's `[MouseCursors]` (`0x1002B125..0x1002B236`).
    pub fn read_pass(&mut self, pass: &IniFile) {
        let Some(section) = pass.section("MouseCursors") else {
            return;
        };
        for key in section.keys() {
            if self.index_of(key).is_none() {
                // strncpy_s(name, 0x20, key, _TRUNCATE) at 0x10007409.
                let name = crate::rules::ini_value::truncate_native_bytes(key, 31);
                self.entries.push((name.to_owned(), NEW_CURSOR));
            }
        }
        for index in 0..self.entries.len() {
            let Some(value) = section.read_name(&self.entries[index].0, 0x800) else {
                continue;
            };
            // 0x100075D3: a value naming a cursor copies that cursor.
            if let Some(source) = self.index_of(value) {
                self.entries[index].1 = self.entries[usize::from(source)].1;
                continue;
            }
            parse_fields(value, &mut self.entries[index].1);
        }
    }
}

/// `Frame,Count,Interval,MiniFrame,MiniCount,HotSpotX,HotSpotY`
/// (`0x100075F1..0x100076BD`): comma tokens (empty ones skipped), each
/// written only when it converts.
fn parse_fields(value: &str, cursor: &mut MouseCursor) {
    let mut tokens = value.split(',').filter(|token| !token.is_empty());
    for slot in [
        &mut cursor.frame,
        &mut cursor.count,
        &mut cursor.interval,
        &mut cursor.mini_frame,
        &mut cursor.mini_count,
    ] {
        if let Some(number) = tokens
            .next()
            .and_then(crate::rules::ini_value::parse_ares_int)
        {
            *slot = number;
        }
    }
    let mut hotspot = |slot: &mut i32, names: [&str; 3]| {
        let Some(token) = tokens.next() else {
            return;
        };
        if let Some(index) = names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(token))
        {
            *slot = [0, FAR, CENTRE][index];
        }
    };
    // 0x10076BC0 and 0x10076C40.
    hotspot(&mut cursor.hotspot_x, ["left", "right", "center"]);
    hotspot(&mut cursor.hotspot_y, ["top", "bottom", "middle"]);
}

/// Ares TechnoTypeExt cursor keys (`+0x590..+0x5A4`, read at
/// `0x1003F27D..0x1003F2EB`) and BuildingTypeExt `Cursor.Spy` (`+0x240`,
/// `0x1000E8D1`), kept as the names they give.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TechnoCursors {
    pub deploy: Option<String>,
    pub no_deploy: Option<String>,
    pub enter: Option<String>,
    pub no_enter: Option<String>,
    pub move_: Option<String>,
    pub no_move: Option<String>,
    pub spy: Option<String>,
}

impl TechnoCursors {
    pub fn read(section: &crate::rules::ini_parser::IniSection) -> Self {
        let read = |key| section.read_name(key, 0x800).map(str::to_owned);
        Self {
            deploy: read("Cursor.Deploy"),
            no_deploy: read("Cursor.NoDeploy"),
            enter: read("Cursor.Enter"),
            no_enter: read("Cursor.NoEnter"),
            move_: read("Cursor.Move"),
            no_move: read("Cursor.NoMove"),
            spy: read("Cursor.Spy"),
        }
    }

    /// Every cursor name these keys give.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        [
            &self.deploy,
            &self.no_deploy,
            &self.enter,
            &self.no_enter,
            &self.move_,
            &self.no_move,
            &self.spy,
        ]
        .into_iter()
        .flatten()
        .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mental Omega's rows: a new name appends after the Ares defaults, a
    /// built-in name is overridden in place (SpyPower gains a rate and a
    /// radar set), and a value naming a cursor copies it.
    #[test]
    fn mouse_cursors_append_new_names_and_override_built_in_rows() {
        let mut catalog = MouseCursorCatalog::default();
        assert_eq!(catalog.len(), 95);
        assert_eq!(catalog.get(ENGINEER_DAMAGE), catalog.get(51));
        catalog.read_pass(&IniFile::from_str(
            "[MouseCursors]\nSpyStar=1002,10,5,100,10,Center,Middle\n\
             SpyPower=229,5,5,100,10,Center,Middle\nCopy=SpyStar\nPartial=7\n",
        ));
        let star = catalog.index_of("spystar").expect("appended");
        assert_eq!(star, 95);
        assert_eq!(catalog.get(star), Some(mid(1002, 10, 5, 100, 10)));
        assert_eq!(catalog.get(43), Some(mid(229, 5, 5, 100, 10)));
        assert_eq!(
            catalog.get(catalog.index_of("Copy").unwrap()),
            catalog.get(star)
        );
        assert_eq!(
            catalog.get(catalog.index_of("Partial").unwrap()),
            Some(row(7, 1, 0, -1, -1, 0, 0))
        );
    }
}
