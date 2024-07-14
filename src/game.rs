use crate::config::init_state;
use crate::player::Player;
use serde::{ser::SerializeSeq, Serialize, Serializer};
use zkwasm_rust_sdk::require;
use zkwasm_rust_sdk::wasm_dbg;

// Custom serializer for `u64` as a string.
pub fn bigint_serializer<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

// Custom serializer for `[u64; 4]` as an array of strings.
pub fn bigint_array_serializer<S>(array: &Vec<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(array.len()))?;
    for &element in array {
        seq.serialize_element(&element.to_string())?;
    }
    seq.end()
}

pub mod event;
pub mod object;
pub mod serialize;
pub mod state;

// This is a standalone game state manipulate module that connets with UI
// controllers and model handlers

const CMD_RUN: u64 = 0;
const CMD_PLACE_TOWER: u64 = 1;
const CMD_CLAIM_TOWER: u64 = 2;
const CMD_MINT_TOWER: u64 = 3;
const CMD_DROP_TOWER: u64 = 4;
const CMD_UPGRADE_TOWER: u64 = 5;
//const CMD_SPAWN: u64 = 3;

fn to_full_obj_id(id: u64) -> [u64; 4] {
    [id, 0xffff, 0xff01, 0xff02]
}

/// Step function receives a encoded command and changes the global state accordingly
pub fn handle_command(commands: &[u64; 4], pid: &[u64; 4]) {
    let command = commands[0] & 0xff;
    let feature = (commands[0] >> 8) & 0xff;
    let nonce = commands[0] >> 16;
    if command == CMD_RUN {
        unsafe { crate::config::GLOBAL.run() };
    } else if command == CMD_PLACE_TOWER {
        let mut player = Player::get(pid).unwrap();
        let objindex = commands[1];
        player.check_and_inc_nonce(nonce);
        unsafe { require(player.owns(objindex)) };
        let pos = commands[2].to_le_bytes();
        let pos = u16::from_le_bytes(pos[0..2].try_into().unwrap());
        state::handle_place_tower(&to_full_obj_id(objindex), pos as usize);
        player.store();
    } else if command == CMD_UPGRADE_TOWER {
        let mut player = Player::get(pid).unwrap();
        player.check_and_inc_nonce(nonce);
        let objindex = commands[1];
        unsafe { require(player.owns(objindex)) };
        state::handle_upgrade_inventory(&to_full_obj_id(objindex));
        player.store();
    } else if command == CMD_MINT_TOWER {
        Player::get_and_check_nonce(pid, nonce);
        let objindex = commands[1];
        let pid = [0, commands[2], commands[3], 0]; // 128bit security strength
        state::handle_update_inventory(&to_full_obj_id(objindex), feature, &pid);
    } else if command == CMD_CLAIM_TOWER {
        let objindex = commands[1];
        state::handle_claim_tower(nonce, &to_full_obj_id(objindex), pid);
    } else if commands[0] == CMD_DROP_TOWER {
        let mut player = Player::get(pid).unwrap();
        player.check_and_inc_nonce(nonce);
        let inventory_index = commands[1];
        unsafe {
            wasm_dbg(inventory_index as u64);
        }
        state::handle_drop_tower(&to_full_obj_id(inventory_index));
        player.store();
    }
}

pub struct State {}

#[derive(Clone, Serialize)]
pub struct UserState<'a> {
    player: Option<Player>,
    global: &'a crate::game::state::State,
}

impl State {
    pub fn get_state(pid: Vec<u64>) -> String {
        //zkwasm_rust_sdk::dbg!("finish loading {:?}", merkle_root);
        let global = unsafe { &crate::config::GLOBAL };
        let player = Player::get(&pid.try_into().unwrap());
        serde_json::to_string(
            &(UserState {
                player,
                global: &global,
            }),
        )
        .unwrap()
    }
    pub fn initialize() {
        init_state()
    }
}

pub struct Transaction {
    pub command: [u64; 4],
}

impl Transaction {
    pub fn decode(params: [u64; 4]) -> Self {
        let command = [params[0], params[1], params[2], params[3]];
        Transaction { command }
    }
    pub fn process(&self, pid: &[u64; 4]) -> bool {
        handle_command(&self.command, pid);
        true
    }
}
