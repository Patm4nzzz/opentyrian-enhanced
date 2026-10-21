//! In-game dashboard: state read by `hud_renderer.gd`, which
//! redraws the tyrian2.c HUD in a `TyrianVga` with the same calls
//! as the C (varz.c:JE_drawShield/JE_drawArmor, nortvars.c:JE_dBar3,
//! tyrian2.c "Weapon Display" / "Power Bar", mainint.c:JE_inGameDisplays).

use godot::classes::image::Format;
use godot::classes::Image;
use godot::prelude::*;
use tyrian_sim::SpecialId;

use crate::vga::TyrianVga;
use crate::TyrianSim;

/// Indices of `TyrianSim.hud_state()` (documented in hud_renderer.gd).
#[godot_api(secondary)]
impl TyrianSim {
    /// HUD state at the current tick, integers:
    /// 0 shield, 1 shield_max, 2 armor (player 1), 3 power (at the time of
    /// the HUD draw), 4/5 front/rear weapon power as read by
    /// "Weapon Display" (two players: front of player 1, rear of
    /// player 2), 6 weapon_mode, 7 itemgraphic of the special weapon (0 =
    /// none), 8 cash of player 1 (clamped i32), 9..14 sidekicks [icongr, ammo,
    /// ammo_max] x 2 of the player carrying them (player 2 in two-player),
    /// 15 superbombs of player 1, 16 two_player_mode, 17 one_player_action,
    /// 18 galaga, 19 smoothies[5], 20 shield, 21 armor, 22 cash, 23 superbombs
    /// of player 2, 24/25 lives (`*lives`) of players 1 and 2,
    /// 26 sidekick_level of player 2, 27 last row occupied at the top of
    /// the play area by a text or a bar (-1 if none).
    #[func]
    fn hud_state(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        // tyrian2.c draws the HUD at the head of the iteration, before the presented frame.
        let w = sim.presented_world();
        let p = &w.players[0];
        let two = w.two_player_mode;
        let sk = &w.players[if two { 1 } else { 0 }];
        let special = w.tables.specials.get(p.items.special.index()).map_or(0, |s| s.itemgraphic as i32);
        let mut v = vec![
            p.shield as i32,
            p.shield_max as i32,
            p.armor as i32,
            w.render.hud_power as i32,
            p.items.weapon[0].power as i32,
            w.players[if two { 1 } else { 0 }].items.weapon[1].power as i32,
            p.weapon_mode as i32,
            if p.items.special > SpecialId(0) { special } else { 0 },
            p.cash.min(i32::MAX as u64) as i32,
        ];
        for i in 0..2 {
            let icon = w.tables.options.get(sk.items.sidekick[i].index()).map_or(0, |o| o.icongr as i32);
            v.extend_from_slice(&[icon, sk.sidekick[i].ammo, sk.sidekick[i].ammo_max]);
        }
        let q = &w.players[1];
        v.extend_from_slice(&[
            p.superbombs as i32,
            two as i32,
            w.one_player_action as i32,
            w.mode.galaga_mode as i32,
            w.background.smoothies[5] as i32,
            q.shield as i32,
            q.armor as i32,
            q.cash.min(i32::MAX as u64) as i32,
            q.superbombs as i32,
            p.lives(0) as i32,
            q.lives(1) as i32,
            q.items.sidekick_level as i32,
            top_text_bottom(w),
        ]);
        PackedInt32Array::from(v)
    }

    /// Dashboard drawings made during the simulation since the
    /// last call, as records of 8 integers:
    /// [0, wipe, shield, armor, (shield, armor) player 1, (shield,
    /// armor) player 2] for varz.c JE_wipeShieldArmorBars / JE_drawShield /
    /// JE_drawArmor; [1, cell, color, 0...] for a cell of the charge
    /// gauge (player_movement.c); [2, 0, ...] for varz.c:JE_drawOptions; [3, index, 0...] for the message of an event 16
    /// (level_events.c:414).
    #[func]
    fn take_hud_ops(&mut self) -> PackedInt32Array {
        use tyrian_sim::world::HudOp;
        let mut out = PackedInt32Array::new();
        let Some(sim) = self.sim.as_mut() else { return out };
        for op in sim.take_hud_ops() {
            let rec = match op {
                HudOp::Bars { wipe, shield, armor, shields, armors } => [
                    0,
                    wipe as i32,
                    shield as i32,
                    armor as i32,
                    shields[0] as i32,
                    armors[0] as i32,
                    shields[1] as i32,
                    armors[1] as i32,
                ],
                HudOp::ChargeCell { level, color } => [1, level as i32, color as i32, 0, 0, 0, 0, 0],
                HudOp::Options => [2, 0, 0, 0, 0, 0, 0, 0],
                HudOp::EventText { index } => [3, index as i32, 0, 0, 0, 0, 0, 0],
            };
            for x in rec {
                out.push(x);
            }
        }
        out
    }

    /// Frame of the charge icon above player 2 (spriteSheet10,
    /// player_movement.c "Super Charge Weapons"), 0 = none this tick.
    #[func]
    fn charge_sprite(&self) -> i32 {
        self.sim.as_ref().map_or(0, |s| s.presented_world().render.drawn_charge_sprite as i32)
    }
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Texts and bars of tyrian2.c drawn into game_screen at the last tick
    /// (`World::drawn_texts`): 0 WARNING (0/1), 1 warningCol, 2 "Secret
    /// Level" (0/1), 3 flash at draw time, 4 countdown (-1 = none),
    /// 5 GAME OVER (0/1), 6 levelTimer, 7 number of boss bars, then
    /// (armor, color) x 2.
    #[func]
    fn drawn_texts(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let w = sim.presented_world();
        let t = &w.render.drawn_texts;
        PackedInt32Array::from(vec![
            t.warning as i32,
            t.warning_col as i32,
            t.secret as i32,
            t.secret_flash as i32,
            t.timer.map_or(-1, |v| v as i32),
            t.game_over as i32,
            w.script.level_timer as i32,
            t.boss_bars as i32,
            t.boss[0].0 as i32,
            t.boss[0].1 as i32,
            t.boss[1].0 as i32,
            t.boss[1].1 as i32,
        ])
    }
}

#[godot_api(secondary)]
impl TyrianVga {
    /// RGBA8 HUD layer laid over the play area drawn by the Godot nodes.
    /// Outside the rectangle [x0, x1) x [y0, y1): surface 0 (VGAScreenSeg), opaque.
    /// Inside: what was drawn over the game (JE_inGameDisplays);
    /// the caller clears surface 1 to 0 and surface 2 to 1 then draws the
    /// same thing on both: a pixel is drawn if and only if the
    /// two surfaces have the same index there (otherwise transparent). No palette
    /// index is thus reserved for transparency. The witness surfaces
    /// are in `game_screen` coordinates: screen pixel x is read there
    /// at x + `src_dx` (enemy_render.c copies game_screen starting at x = 24).
    #[func]
    fn hud_overlay(&self, x0: i32, y0: i32, x1: i32, y1: i32, src_dx: i32) -> Option<Gd<Image>> {
        let (seg, a, b) = (&self.surfaces[0], &self.surfaces[1], &self.surfaces[2]);
        let pal = &self.palette.colors;
        let mut out = Vec::with_capacity(320 * 200 * 4);
        for y in 0..200i32 {
            for x in 0..320i32 {
                let i = (y * 320 + x) as usize;
                let inside = x >= x0 && x < x1 && y >= y0 && y < y1;
                let (idx, alpha) = if !inside {
                    (seg.pixels[i], 255)
                } else if let Some(j) =
                    ((x + src_dx) < 320).then_some(i + src_dx as usize).filter(|&j| a.pixels[j] == b.pixels[j])
                {
                    (a.pixels[j], 255)
                } else {
                    (0, 0)
                };
                let c = pal[idx as usize];
                out.extend_from_slice(&[c[0], c[1], c[2], alpha]);
            }
        }
        Image::create_from_data(320, 200, false, Format::RGBA8, &PackedByteArray::from(out))
    }

    /// `vga256d.c:JE_barShade` on the already-drawn pixels of the witness
    /// surfaces 1 and 2 (money, texts): `stats_display` and
    /// `achievements_display` darken what is under their panel.
    /// Surface coordinates (x 24..287 = play area). Pixels not
    /// drawn (different between the two surfaces) stay transparent.
    #[func]
    fn shade_overlay(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (head, tail) = self.surfaces.split_at_mut(2);
        let (a, b) = (&mut head[1].pixels, &mut tail[0].pixels);
        for y in y0.max(0)..=y1.min(199) {
            for x in x0.max(0)..=x1.min(319) {
                let i = (y * 320 + x) as usize;
                if a[i] == b[i] {
                    let v = ((a[i] & 0x0f) >> 1) | (a[i] & 0xf0);
                    a[i] = v;
                    b[i] = v;
                }
            }
        }
    }

    /// `enemy_render.c:JE_starShowVGA` code 2 (lamp centered on player 1)
    /// applied to what JE_inGameDisplays drew in the play area of the
    /// witness surfaces 1 and 2 (money, lives, special weapon: equal pixels on
    /// both, see `hud_overlay`), in game_screen coordinates (play
    /// area starting at x = `dx`): darkened like the scenery. Pixels not
    /// drawn stay different, hence transparent.
    #[func]
    fn lamp_overlay(&mut self, dx: i32, lightx: i32, lighty: i32) {
        let (head, tail) = self.surfaces.split_at_mut(2);
        let (a, b) = (&mut head[1].pixels, &mut tail[0].pixels);
        for r in 0..crate::playfield::PLAY_H {
            for c in 0..crate::playfield::PLAY_W {
                let i = r * 320 + c + dx as usize;
                if a[i] == b[i] {
                    let v = crate::playfield::lamp_index(a[i], c, r, lightx, lighty);
                    a[i] = v;
                    b[i] = v;
                }
            }
        }
    }
}

/// Last row occupied at the top of the play area by the countdown
/// (tyrian2.c "Level Timer": 10 px glyphs at y 2, shadow +2), the boss
/// bar (level_helpers.c:draw_boss_bar, up to y + 5) or "Secret Level"
/// (FONT_SHAPES at y 10), -1 if none: the achievement banner is placed
/// below it (intended gap, achievements.c:hud_top_text_bottom).
pub(crate) fn top_text_bottom(w: &tyrian_sim::world::World) -> i32 {
    let t = &w.render.drawn_texts;
    let mut bottom = -1;
    if t.timer.is_some() {
        bottom = bottom.max(14);
    }
    if t.boss_bars > 0 {
        bottom = bottom.max(if w.script.level_timer { 15 } else { 7 } + 5);
    }
    if t.secret {
        bottom = bottom.max(30);
    }
    bottom
}
