//! Differential test for the object-config loaders against the C++ oracle. The
//! golden (one `label hash` line per group) is produced by the real C++
//! `Common::load` (which reads the .cfg files + resolves cross-refs); the Rust
//! loaders must reproduce every FNV-1a digest from the same shipped
//! `data/TC/openliero`. Digest byte layout matches object_dump.cpp exactly (LE
//! ints; bools = 1 byte; strings = u32 LE length + bytes; resolved cross-refs as
//! i32). The encode_* field order mirrors LoadWeaponConfig / LoadNObjectConfig /
//! LoadSObjectConfig.

use std::collections::HashMap;

use assets::object::{NObjectType, Objects, SObjectType, Weapon};
use assets::tc::TcConfig;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn push_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_i32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_bool(b: &mut Vec<u8>, v: bool) {
    b.push(if v { 1 } else { 0 });
}
fn push_str(b: &mut Vec<u8>, s: &str) {
    push_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}

fn encode_weapon(b: &mut Vec<u8>, w: &Weapon) {
    push_i32(b, w.id);
    push_str(b, &w.id_str);
    push_str(b, &w.name);
    push_bool(b, w.affect_by_worm);
    push_bool(b, w.shadow);
    push_bool(b, w.laser_sight);
    push_bool(b, w.play_reload_sound);
    push_bool(b, w.worm_explode);
    push_bool(b, w.expl_ground);
    push_bool(b, w.worm_collide);
    push_bool(b, w.collide_with_objects);
    push_bool(b, w.affect_by_explosions);
    push_bool(b, w.loop_anim);
    push_i32(b, w.detect_distance);
    push_i32(b, w.blow_away);
    push_i32(b, w.gravity);
    push_i32(b, w.launch_sound);
    push_bool(b, w.loop_sound);
    push_i32(b, w.explo_sound);
    push_i32(b, w.speed);
    push_i32(b, w.add_speed);
    push_i32(b, w.distribution);
    push_i32(b, w.parts);
    push_i32(b, w.recoil);
    push_i32(b, w.mult_speed);
    push_i32(b, w.delay);
    push_i32(b, w.loading_time);
    push_i32(b, w.ammo);
    push_i32(b, w.dirt_effect);
    push_i32(b, w.leave_shells);
    push_i32(b, w.leave_shell_delay);
    push_i32(b, w.fire_cone);
    push_i32(b, w.bounce);
    push_i32(b, w.time_to_explo);
    push_i32(b, w.time_to_explo_v);
    push_i32(b, w.hit_damage);
    push_i32(b, w.blood_on_hit);
    push_i32(b, w.start_frame);
    push_i32(b, w.num_frames);
    push_i32(b, w.shot_type);
    push_i32(b, w.color_bullets);
    push_i32(b, w.splinter_amount);
    push_i32(b, w.splinter_colour);
    push_i32(b, w.splinter_type);
    push_i32(b, w.splinter_scatter);
    push_i32(b, w.obj_trail_type);
    push_i32(b, w.obj_trail_delay);
    push_i32(b, w.part_trail_type);
    push_i32(b, w.part_trail_obj);
    push_i32(b, w.part_trail_delay);
    push_i32(b, w.create_on_exp);
    push_bool(b, w.chain_explosion);
}

fn encode_nobject(b: &mut Vec<u8>, n: &NObjectType) {
    push_i32(b, n.id);
    push_str(b, &n.id_str);
    push_bool(b, n.worm_explode);
    push_bool(b, n.expl_ground);
    push_bool(b, n.worm_destroy);
    push_bool(b, n.draw_on_map);
    push_bool(b, n.affect_by_explosions);
    push_bool(b, n.blood_trail);
    push_i32(b, n.detect_distance);
    push_i32(b, n.gravity);
    push_i32(b, n.speed);
    push_i32(b, n.speed_v);
    push_i32(b, n.distribution);
    push_i32(b, n.blow_away);
    push_i32(b, n.bounce);
    push_i32(b, n.hit_damage);
    push_i32(b, n.blood_on_hit);
    push_i32(b, n.start_frame);
    push_i32(b, n.num_frames);
    push_i32(b, n.color_bullets);
    push_i32(b, n.create_on_exp);
    push_i32(b, n.dirt_effect);
    push_i32(b, n.splinter_amount);
    push_i32(b, n.splinter_colour);
    push_i32(b, n.splinter_type);
    push_i32(b, n.blood_trail_delay);
    push_i32(b, n.leave_obj);
    push_i32(b, n.leave_obj_delay);
    push_i32(b, n.time_to_explo);
    push_i32(b, n.time_to_explo_v);
}

fn encode_sobject(b: &mut Vec<u8>, s: &SObjectType) {
    push_i32(b, s.id);
    push_str(b, &s.id_str);
    push_bool(b, s.shadow);
    push_i32(b, s.start_sound);
    push_i32(b, s.num_sounds);
    push_i32(b, s.anim_delay);
    push_i32(b, s.start_frame);
    push_i32(b, s.num_frames);
    push_i32(b, s.detect_range);
    push_i32(b, s.damage);
    push_i32(b, s.blow_away);
    push_i32(b, s.shake);
    push_i32(b, s.flash);
    push_i32(b, s.dirt_effect);
}

fn parse_golden(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let label = it.next().unwrap().to_string();
            let hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (label, hash)
        })
        .collect()
}

#[test]
fn object_configs_match_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/object.txt"
    )));

    // 1e-1 gives us the ordered type-name lists (the resolution lists + read order).
    let tc_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
    let tc = TcConfig::load(tc_bytes).expect("tc.cfg parses");

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
    let objs = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{root}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    let want = |k: &str| *golden.get(k).unwrap_or_else(|| panic!("missing golden {k}"));

    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.weapons.len() as u32);
        for w in &objs.weapons {
            encode_weapon(&mut b, w);
        }
        assert_eq!(fnv1a(&b), want("weapons"));
    }
    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.nobject_types.len() as u32);
        for n in &objs.nobject_types {
            encode_nobject(&mut b, n);
        }
        assert_eq!(fnv1a(&b), want("nobjects"));
    }
    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.sobject_types.len() as u32);
        for s in &objs.sobject_types {
            encode_sobject(&mut b, s);
        }
        assert_eq!(fnv1a(&b), want("sobjects"));
    }
}
