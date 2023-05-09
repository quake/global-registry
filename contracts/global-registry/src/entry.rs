// Import from `core` instead of from `std` since we are in no-std mode
use core::{ops::Deref, result::Result};

// Import CKB syscalls and structures
// https://docs.rs/ckb-std/
use ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::*,
    high_level::{
        load_cell_data, load_cell_lock, load_cell_type_hash, load_input, load_script,
        load_script_hash,
    },
    syscalls::{self, SysError},
};

use crate::error::Error;

pub fn main() -> Result<(), Error> {
    if is_init() {
        validate_init_hash()
    } else {
        validate_linked_list()
    }
}

// check if we are initializing the global registry
fn is_init() -> bool {
    let mut buf = [0u8; 0];
    // load cell to a zero-length buffer must be failed, we are using this tricky way to check if input group is empty, which means we are initializing the global registry
    match syscalls::load_cell(&mut buf, 0, 0, Source::GroupInput).unwrap_err() {
        SysError::LengthNotEnough(_) => false,
        SysError::IndexOutOfBound => true,
        _ => unreachable!("is_init"),
    }
}

// check if the init hash is correct, which is the hash of the first input and the index of the first output with the same type script
fn validate_init_hash() -> Result<(), Error> {
    let current_script = load_script()?;
    let first_input = load_input(0, Source::Input)?;
    let first_output_index = load_first_output_index()?;
    let mut hash = [0; 32];
    let mut blake2b = blake2b_rs::Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build();
    blake2b.update(first_input.as_slice());
    blake2b.update(&first_output_index.to_le_bytes());
    blake2b.finalize(&mut hash);

    if current_script.args().raw_data().deref() == hash {
        Ok(())
    } else {
        Err(Error::InvalidInitHash)
    }
}

// check if the linked list is valid
fn validate_linked_list() -> Result<(), Error> {
    let mut i = 0;
    let mut o = 0;
    while let Ok(script) = load_cell_lock(i, Source::GroupInput) {
        if script.args().len() < 32 {
            return Err(Error::InvalidArgsLength);
        }
        let mut input_start: [u8; 32] = script.args().raw_data()[0..32].try_into().unwrap();

        let data = load_cell_data(i, Source::GroupInput)?;
        if data.len() < 32 {
            return Err(Error::InvalidDataLength);
        }
        let input_end: [u8; 32] = data[0..32].try_into().unwrap();

        loop {
            match load_cell_lock(o, Source::GroupOutput) {
                Ok(script) => {
                    if script.args().len() < 32 {
                        return Err(Error::InvalidArgsLength);
                    }
                    let output_start: [u8; 32] =
                        script.args().raw_data()[0..32].try_into().unwrap();
                    if output_start != input_start {
                        return Err(Error::InvalidLinkedList);
                    }

                    let data = load_cell_data(o, Source::GroupOutput)?;
                    if data.len() < 32 {
                        return Err(Error::InvalidDataLength);
                    }
                    let output_end: [u8; 32] = data[0..32].try_into().unwrap();

                    o += 1;
                    if output_end != input_end {
                        input_start = output_end;
                    } else {
                        break;
                    }
                }
                Err(_) => {
                    return Err(Error::InvalidLinkedList);
                }
            }
        }
        i += 1;
    }

    // check if all the outputs are visited
    match load_cell_lock(o, Source::GroupOutput) {
        Err(SysError::IndexOutOfBound) => Ok(()),
        _ => Err(Error::InvalidLinkedList),
    }
}

fn load_first_output_index() -> Result<usize, Error> {
    let current_script_hash = load_script_hash()?;

    let mut i = 0;
    while let Some(type_hash) = load_cell_type_hash(i, Source::Output)? {
        if type_hash == current_script_hash {
            return Ok(i);
        }
        i += 1
    }
    unreachable!()
}
