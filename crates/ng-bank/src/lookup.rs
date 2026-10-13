// <META - FILE SUMMARY - Preset lookup with the GM fallback chain and the coverage report (R03.DESIGN).>

use ng_core::LaneKind;

use crate::image::{PresetEntry, SoundbankImage};

/// The GM percussion bank (C0.6).
pub const DRUM_BANK: u16 = 128;

/// Presets a bank actually contains (R03.DESIGN "coverage() reports present GM 0-127 and kits").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// True when some non-drum preset answers to this GM program number.
    pub melodic: [bool; 128],
    /// True when some preset on bank 128 answers to this kit number.
    pub kits: [bool; 128],
    /// Every melodic (bank, program) in file order.
    pub melodic_slots: Vec<(u16, u16)>,
    /// Every kit program number in file order.
    pub kit_slots: Vec<u16>,
    pub melodic_count: usize,
    pub kit_count: usize,
}

impl Default for Coverage {
    fn default() -> Self {
        Self {
            melodic: [false; 128],
            kits: [false; 128],
            melodic_slots: Vec::new(),
            kit_slots: Vec::new(),
            melodic_count: 0,
            kit_count: 0,
        }
    }
}

impl Coverage {
    /// GM programs 0..128 that the bank does not answer to.
    pub fn missing_melodic(&self) -> Vec<u16> {
        (0..128).filter(|p| !self.melodic[*p as usize]).collect()
    }

    /// Kit numbers the bank does not answer to.
    pub fn missing_kits(&self) -> Vec<u16> {
        (0..128).filter(|k| !self.kits[*k as usize]).collect()
    }
}

/// The fallback chain R03.DESIGN asks for: melodic (0, p) then (0, 0); drum (128, kit) then
/// (128, 0). The exact match is always tried first so a bank in another bank number still works.
pub fn lookup_chain(bank: u16, program: u16) -> Vec<(u16, u16)> {
    let mut chain = vec![(bank, program)];
    let drum = bank == DRUM_BANK;
    let next = if drum { (DRUM_BANK, 0) } else { (0, 0) };
    let mid = if drum { (DRUM_BANK, program) } else { (0, program) };
    for candidate in [mid, next] {
        if !chain.contains(&candidate) {
            chain.push(candidate);
        }
    }
    chain
}

impl SoundbankImage {
    /// Index of the preset a note should play, following the fallback chain.
    pub fn lookup(&self, kind: LaneKind, bank: u16, program: u16) -> Option<usize> {
        let start_bank = if kind == LaneKind::Drum { DRUM_BANK } else { bank };
        lookup_chain(start_bank, program)
            .iter()
            .find_map(|(b, p)| self.lookup_exact(*b, *p))
    }

    /// Index of the preset answering to exactly this (bank, program), if the bank has one.
    /// SF2 2.01 sec 7.2: on duplicates the first record in phdr is the active preset.
    pub fn lookup_exact(&self, bank: u16, program: u16) -> Option<usize> {
        self.presets
            .iter()
            .position(|p: &PresetEntry| p.bank == bank && p.program == program)
    }

    pub fn coverage(&self) -> Coverage {
        let mut c = Coverage::default();
        for p in &self.presets {
            let program = p.program;
            if p.bank == DRUM_BANK {
                if program < 128 {
                    c.kits[program as usize] = true;
                    c.kit_slots.push(program);
                    c.kit_count += 1;
                }
                continue;
            }
            if program < 128 {
                c.melodic[program as usize] = true;
                c.melodic_count += 1;
            }
            c.melodic_slots.push((p.bank, p.program));
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_follow_the_design() {
        assert_eq!(lookup_chain(0, 42), vec![(0, 42), (0, 0)]);
        assert_eq!(lookup_chain(3, 42), vec![(3, 42), (0, 42), (0, 0)]);
        assert_eq!(lookup_chain(DRUM_BANK, 7), vec![(DRUM_BANK, 7), (DRUM_BANK, 0)]);
        assert_eq!(lookup_chain(0, 0), vec![(0, 0)]);
    }
}