//! Item, weapon and enemy tables (`episodes.c:JE_loadItemDat`).
//!
//! The same block ("item block", spec § 2.2) is found:
//! - in `tyrian.hdt` starting at `episode1DataLoc` (episodes 1 to 3);
//! - in the last entry of `tyrian4.lvl` / `tyrian5.lvl` (episodes 4 and 5).
//!
//! Record counts are fixed by the format (C constants),
//! not by `item_num`.

use crate::crypt::{bytes_to_text, EncryptedStringFile};
use crate::io::{FormatError, Reader, Result, Writer};
use serde::{Serialize, Serializer};

/// Last weapon index of bank 1 (`lvlmast.h:WEAP_END1`).
pub const WEAP_END1: usize = 818;
/// First weapon index of bank 2 (`WEAP_START2`).
pub const WEAP_START2: usize = 1000;
/// Last weapon index of bank 2 in the file (`HDT_WEAP_BANK2_END`).
pub const HDT_WEAP_BANK2_END: usize = 1818;
pub const HDT_PORT_COUNT: usize = 61;
pub const HDT_SPECIAL_COUNT: usize = 55;
pub const HDT_POWER_COUNT: usize = 7;
pub const HDT_SHIP_COUNT: usize = 19;
pub const HDT_OPTION_COUNT: usize = 38;
pub const HDT_SHIELD_COUNT: usize = 12;
/// Last enemy index of bank 1 (`ENEMY_END1`).
pub const ENEMY_END1: usize = 850;
/// First enemy index of bank 2 (`ENEMY_START2`).
pub const ENEMY_START2: usize = 1001;
/// Last enemy index (`ENEMY_NUM`).
pub const ENEMY_NUM: usize = 1850;

/// Number of weapons in the file (819 + 819).
pub const HDT_WEAPON_RECORDS: usize = (WEAP_END1 + 1) + (HDT_WEAP_BANK2_END - WEAP_START2 + 1);
/// Number of enemies in the file (851 + 850).
pub const HDT_ENEMY_RECORDS: usize = (ENEMY_END1 + 1) + (ENEMY_NUM - ENEMY_START2 + 1);

/// C index (`weapons[i]`) of weapon record number `record`.
pub fn weapon_id(record: usize) -> usize {
    if record <= WEAP_END1 {
        record
    } else {
        WEAP_START2 + (record - (WEAP_END1 + 1))
    }
}

/// C index (`enemyDat[i]`) of enemy record number `record`.
pub fn enemy_id(record: usize) -> usize {
    if record <= ENEMY_END1 {
        record
    } else {
        ENEMY_START2 + (record - (ENEMY_END1 + 1))
    }
}

/// Pascal `string[30]` name: `u8 len` + 30 fixed bytes (original padding kept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PascalName {
    pub len: u8,
    pub bytes: [u8; 30],
}

impl PascalName {
    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self { len: r.u8()?, bytes: r.array()? })
    }

    fn write(&self, w: &mut Writer) {
        w.u8(self.len);
        w.bytes(&self.bytes);
    }

    /// Name as seen by the C code: cut at `min(len,30)`, at the first NUL, then
    /// `trim_name` (trailing spaces removed, first character kept).
    pub fn text(&self) -> String {
        let n = (self.len as usize).min(30);
        let mut s = &self.bytes[..n];
        if let Some(z) = s.iter().position(|&b| b == 0) {
            s = &s[..z];
        }
        let mut end = s.len();
        while end > 1 && s[end - 1] == b' ' {
            end -= 1;
        }
        bytes_to_text(&s[..end])
    }
}

impl Serialize for PascalName {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.text())
    }
}

/// `JE_WeaponType` (80 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Weapon {
    pub drain: u16,
    pub shotrepeat: u8,
    pub multi: u8,
    pub weapani: u16,
    pub max: u8,
    pub tx: u8,
    pub ty: u8,
    pub aim: u8,
    pub attack: [u8; 8],
    pub del: [u8; 8],
    pub sx: [i8; 8],
    pub sy: [i8; 8],
    pub bx: [i8; 8],
    pub by: [i8; 8],
    pub sg: [u16; 8],
    pub acceleration: i8,
    pub accelerationx: i8,
    pub circlesize: u8,
    pub sound: u8,
    pub trail: u8,
    pub shipblastfilter: u8,
}

impl Weapon {
    pub const SIZE: usize = 80;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            drain: r.u16()?,
            shotrepeat: r.u8()?,
            multi: r.u8()?,
            weapani: r.u16()?,
            max: r.u8()?,
            tx: r.u8()?,
            ty: r.u8()?,
            aim: r.u8()?,
            attack: r.u8_array()?,
            del: r.u8_array()?,
            sx: r.i8_array()?,
            sy: r.i8_array()?,
            bx: r.i8_array()?,
            by: r.i8_array()?,
            sg: r.u16_array()?,
            acceleration: r.i8()?,
            accelerationx: r.i8()?,
            circlesize: r.u8()?,
            sound: r.u8()?,
            trail: r.u8()?,
            shipblastfilter: r.u8()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        w.u16(self.drain);
        w.u8(self.shotrepeat);
        w.u8(self.multi);
        w.u16(self.weapani);
        w.u8(self.max);
        w.u8(self.tx);
        w.u8(self.ty);
        w.u8(self.aim);
        w.bytes(&self.attack);
        w.bytes(&self.del);
        w.i8_slice(&self.sx);
        w.i8_slice(&self.sy);
        w.i8_slice(&self.bx);
        w.i8_slice(&self.by);
        w.u16_slice(&self.sg);
        w.i8(self.acceleration);
        w.i8(self.accelerationx);
        w.u8(self.circlesize);
        w.u8(self.sound);
        w.u8(self.trail);
        w.u8(self.shipblastfilter);
    }
}

/// `JE_WeaponPortType` (82 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeaponPort {
    pub name: PascalName,
    pub opnum: u8,
    /// `op[mode][power level]`: weapon indices.
    pub op: [[u16; 11]; 2],
    pub cost: u16,
    pub itemgraphic: u16,
    pub poweruse: u16,
}

impl WeaponPort {
    pub const SIZE: usize = 82;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            name: PascalName::read(r)?,
            opnum: r.u8()?,
            op: [r.u16_array()?, r.u16_array()?],
            cost: r.u16()?,
            itemgraphic: r.u16()?,
            poweruse: r.u16()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u8(self.opnum);
        w.u16_slice(&self.op[0]);
        w.u16_slice(&self.op[1]);
        w.u16(self.cost);
        w.u16(self.itemgraphic);
        w.u16(self.poweruse);
    }
}

/// `JE_SpecialType` (37 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Special {
    pub name: PascalName,
    pub itemgraphic: u16,
    pub pwr: u8,
    pub stype: u8,
    pub wpn: u16,
}

impl Special {
    pub const SIZE: usize = 37;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self { name: PascalName::read(r)?, itemgraphic: r.u16()?, pwr: r.u8()?, stype: r.u8()?, wpn: r.u16()? })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u16(self.itemgraphic);
        w.u8(self.pwr);
        w.u8(self.stype);
        w.u16(self.wpn);
    }
}

/// `JE_PowerType` — generator (37 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Generator {
    pub name: PascalName,
    pub itemgraphic: u16,
    pub power: u8,
    pub speed: i8,
    pub cost: u16,
}

impl Generator {
    pub const SIZE: usize = 37;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self { name: PascalName::read(r)?, itemgraphic: r.u16()?, power: r.u8()?, speed: r.i8()?, cost: r.u16()? })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u16(self.itemgraphic);
        w.u8(self.power);
        w.i8(self.speed);
        w.u16(self.cost);
    }
}

/// `JE_ShipType` (41 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ship {
    pub name: PascalName,
    pub shipgraphic: u16,
    pub itemgraphic: u16,
    pub ani: u8,
    pub spd: i8,
    pub dmg: u8,
    pub cost: u16,
    pub bigshipgraphic: u8,
}

impl Ship {
    pub const SIZE: usize = 41;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            name: PascalName::read(r)?,
            shipgraphic: r.u16()?,
            itemgraphic: r.u16()?,
            ani: r.u8()?,
            spd: r.i8()?,
            dmg: r.u8()?,
            cost: r.u16()?,
            bigshipgraphic: r.u8()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u16(self.shipgraphic);
        w.u16(self.itemgraphic);
        w.u8(self.ani);
        w.i8(self.spd);
        w.u8(self.dmg);
        w.u16(self.cost);
        w.u8(self.bigshipgraphic);
    }
}

/// `JE_OptionType` — sidekick (86 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sidekick {
    pub name: PascalName,
    pub pwr: u8,
    pub itemgraphic: u16,
    pub cost: u16,
    pub tr: u8,
    pub option: u8,
    pub opspd: i8,
    pub ani: u8,
    pub gr: [u16; 20],
    pub wport: u8,
    pub wpnum: u16,
    pub ammo: u8,
    /// Raw byte from `fread_bool_die` (≠ 0 → true).
    pub stop: u8,
    pub icongr: u8,
}

impl Sidekick {
    pub const SIZE: usize = 86;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            name: PascalName::read(r)?,
            pwr: r.u8()?,
            itemgraphic: r.u16()?,
            cost: r.u16()?,
            tr: r.u8()?,
            option: r.u8()?,
            opspd: r.i8()?,
            ani: r.u8()?,
            gr: r.u16_array()?,
            wport: r.u8()?,
            wpnum: r.u16()?,
            ammo: r.u8()?,
            stop: r.u8()?,
            icongr: r.u8()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u8(self.pwr);
        w.u16(self.itemgraphic);
        w.u16(self.cost);
        w.u8(self.tr);
        w.u8(self.option);
        w.i8(self.opspd);
        w.u8(self.ani);
        w.u16_slice(&self.gr);
        w.u8(self.wport);
        w.u16(self.wpnum);
        w.u8(self.ammo);
        w.u8(self.stop);
        w.u8(self.icongr);
    }

    pub fn stop(&self) -> bool {
        self.stop != 0
    }
}

/// `JE_ShieldType` (37 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shield {
    pub name: PascalName,
    pub tpwr: u8,
    pub mpwr: u8,
    pub itemgraphic: u16,
    pub cost: u16,
}

impl Shield {
    pub const SIZE: usize = 37;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self { name: PascalName::read(r)?, tpwr: r.u8()?, mpwr: r.u8()?, itemgraphic: r.u16()?, cost: r.u16()? })
    }

    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.u8(self.tpwr);
        w.u8(self.mpwr);
        w.u16(self.itemgraphic);
        w.u16(self.cost);
    }
}

/// `JE_EnemyDatType` (77 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnemyDat {
    pub ani: u8,
    pub tur: [u8; 3],
    pub freq: [u8; 3],
    pub xmove: i8,
    pub ymove: i8,
    pub xaccel: i8,
    pub yaccel: i8,
    pub xcaccel: i8,
    pub ycaccel: i8,
    pub startx: i16,
    pub starty: i16,
    pub startxc: i8,
    pub startyc: i8,
    pub armor: u8,
    pub esize: u8,
    pub egraphic: [u16; 20],
    pub explosiontype: u8,
    pub animate: u8,
    pub shapebank: u8,
    pub xrev: i8,
    pub yrev: i8,
    pub dgr: u16,
    pub dlevel: i8,
    pub dani: i8,
    pub elaunchfreq: u8,
    pub elaunchtype: u16,
    pub value: i16,
    pub eenemydie: u16,
}

impl EnemyDat {
    pub const SIZE: usize = 77;

    pub fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            ani: r.u8()?,
            tur: r.u8_array()?,
            freq: r.u8_array()?,
            xmove: r.i8()?,
            ymove: r.i8()?,
            xaccel: r.i8()?,
            yaccel: r.i8()?,
            xcaccel: r.i8()?,
            ycaccel: r.i8()?,
            startx: r.i16()?,
            starty: r.i16()?,
            startxc: r.i8()?,
            startyc: r.i8()?,
            armor: r.u8()?,
            esize: r.u8()?,
            egraphic: r.u16_array()?,
            explosiontype: r.u8()?,
            animate: r.u8()?,
            shapebank: r.u8()?,
            xrev: r.i8()?,
            yrev: r.i8()?,
            dgr: r.u16()?,
            dlevel: r.i8()?,
            dani: r.i8()?,
            elaunchfreq: r.u8()?,
            elaunchtype: r.u16()?,
            value: r.i16()?,
            eenemydie: r.u16()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u8(self.ani);
        w.bytes(&self.tur);
        w.bytes(&self.freq);
        w.i8(self.xmove);
        w.i8(self.ymove);
        w.i8(self.xaccel);
        w.i8(self.yaccel);
        w.i8(self.xcaccel);
        w.i8(self.ycaccel);
        w.i16(self.startx);
        w.i16(self.starty);
        w.i8(self.startxc);
        w.i8(self.startyc);
        w.u8(self.armor);
        w.u8(self.esize);
        w.u16_slice(&self.egraphic);
        w.u8(self.explosiontype);
        w.u8(self.animate);
        w.u8(self.shapebank);
        w.i8(self.xrev);
        w.i8(self.yrev);
        w.u16(self.dgr);
        w.i8(self.dlevel);
        w.i8(self.dani);
        w.u8(self.elaunchfreq);
        w.u16(self.elaunchtype);
        w.i16(self.value);
        w.u16(self.eenemydie);
    }
}

/// Complete item block (`JE_loadItemDat`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemTables {
    /// `itemNum[7]`: informational, not used by the C code for reading.
    pub item_num: [u16; 7],
    /// 1,638 weapons; C index via [`weapon_id`] (0..=818 then 1000..=1818).
    pub weapons: Vec<Weapon>,
    pub weapon_ports: Vec<WeaponPort>,
    pub specials: Vec<Special>,
    pub generators: Vec<Generator>,
    pub ships: Vec<Ship>,
    pub sidekicks: Vec<Sidekick>,
    pub shields: Vec<Shield>,
    /// 1,701 enemies; C index via [`enemy_id`] (0..=850 then 1001..=1850).
    pub enemies: Vec<EnemyDat>,
    /// Bytes not read after the last enemy (77 bytes in the shipped data).
    #[serde(skip)]
    pub tail: Vec<u8>,
}

/// Size of the block actually read by the C code (excluding the tail).
pub const ITEM_BLOCK_SIZE: usize = 14
    + HDT_WEAPON_RECORDS * Weapon::SIZE
    + HDT_PORT_COUNT * WeaponPort::SIZE
    + HDT_SPECIAL_COUNT * Special::SIZE
    + HDT_POWER_COUNT * Generator::SIZE
    + HDT_SHIP_COUNT * Ship::SIZE
    + HDT_OPTION_COUNT * Sidekick::SIZE
    + HDT_SHIELD_COUNT * Shield::SIZE
    + HDT_ENEMY_RECORDS * EnemyDat::SIZE;

fn read_n<T>(r: &mut Reader, n: usize, f: fn(&mut Reader) -> Result<T>) -> Result<Vec<T>> {
    (0..n).map(|_| f(r)).collect()
}

impl ItemTables {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let item_num = r.u16_array()?;
        let weapons = read_n(&mut r, HDT_WEAPON_RECORDS, Weapon::read)?;
        let weapon_ports = read_n(&mut r, HDT_PORT_COUNT, WeaponPort::read)?;
        let specials = read_n(&mut r, HDT_SPECIAL_COUNT, Special::read)?;
        let generators = read_n(&mut r, HDT_POWER_COUNT, Generator::read)?;
        let ships = read_n(&mut r, HDT_SHIP_COUNT, Ship::read)?;
        let sidekicks = read_n(&mut r, HDT_OPTION_COUNT, Sidekick::read)?;
        let shields = read_n(&mut r, HDT_SHIELD_COUNT, Shield::read)?;
        let enemies = read_n(&mut r, HDT_ENEMY_RECORDS, EnemyDat::read)?;
        let tail = r.rest();
        Ok(Self { item_num, weapons, weapon_ports, specials, generators, ships, sidekicks, shields, enemies, tail })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u16_slice(&self.item_num);
        self.weapons.iter().for_each(|x| x.write(&mut w));
        self.weapon_ports.iter().for_each(|x| x.write(&mut w));
        self.specials.iter().for_each(|x| x.write(&mut w));
        self.generators.iter().for_each(|x| x.write(&mut w));
        self.ships.iter().for_each(|x| x.write(&mut w));
        self.sidekicks.iter().for_each(|x| x.write(&mut w));
        self.shields.iter().for_each(|x| x.write(&mut w));
        self.enemies.iter().for_each(|x| x.write(&mut w));
        w.bytes(&self.tail);
        w.into_inner()
    }

    /// Weapon by C index (`weapons[id]`), `None` outside the two banks.
    pub fn weapon(&self, id: usize) -> Option<&Weapon> {
        let rec = if id <= WEAP_END1 {
            id
        } else if (WEAP_START2..=HDT_WEAP_BANK2_END).contains(&id) {
            id - WEAP_START2 + WEAP_END1 + 1
        } else {
            return None;
        };
        self.weapons.get(rec)
    }

    /// Enemy by C index (`enemyDat[id]`), `None` outside the two banks.
    pub fn enemy(&self, id: usize) -> Option<&EnemyDat> {
        let rec = if id <= ENEMY_END1 {
            id
        } else if (ENEMY_START2..=ENEMY_NUM).contains(&id) {
            id - ENEMY_START2 + ENEMY_END1 + 1
        } else {
            return None;
        };
        self.enemies.get(rec)
    }
}

/// Help area of `tyrian.hdt`: encrypted strings if they cover
/// the area exactly, otherwise raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpArea {
    Strings(EncryptedStringFile),
    Raw(Vec<u8>),
}

/// Complete `tyrian.hdt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdtFile {
    /// Offset of the item block (`episode1DataLoc`, 21,174).
    pub episode1_data_loc: i32,
    /// Bytes 4..episode1_data_loc (original help texts, not read by the C code).
    pub help: HelpArea,
    pub items: ItemTables,
}

impl HdtFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let loc = r.i32()?;
        if loc < 4 || loc as usize > data.len() {
            return Err(FormatError::new(format!("tyrian.hdt: invalid episode1DataLoc {loc}")));
        }
        let help_bytes = &data[4..loc as usize];
        let help = match EncryptedStringFile::parse(help_bytes) {
            Ok(f) if f.write() == help_bytes => HelpArea::Strings(f),
            _ => HelpArea::Raw(help_bytes.to_vec()),
        };
        let items = ItemTables::parse(&data[loc as usize..])?;
        Ok(Self { episode1_data_loc: loc, help, items })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.i32(self.episode1_data_loc);
        match &self.help {
            HelpArea::Strings(f) => w.bytes(&f.write()),
            HelpArea::Raw(b) => w.bytes(b),
        }
        w.bytes(&self.items.write());
        w.into_inner()
    }
}
