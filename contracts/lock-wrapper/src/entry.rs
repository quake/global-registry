// Import from `core` instead of from `std` since we are in no-std mode
use core::{cmp::Ordering, result::Result};

// Import heap related library from `alloc`
// https://doc.rust-lang.org/alloc/index.html
use alloc::vec::Vec;

// Import CKB syscalls and structures
// https://docs.rs/ckb-std/
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{packed::Script, prelude::*},
    debug,
    high_level::{
        load_cell, load_cell_data, load_cell_lock, load_cell_type_hash, load_script, QueryIter,
    },
};

use crate::error::Error;

pub fn main() -> Result<(), Error> {
    let current_script = load_script()?;
    if current_script.args().len() != 64 {
        return Err(Error::InvalidArgsLength);
    }

    if is_delegate_to_wrapped(&current_script) {
        exec_wrapped_script(&current_script)
    } else {
        validate_config_value(&current_script)
    }
}

fn is_delegate_to_wrapped(current_script: &Script) -> bool {
    let global_registry_script_hash: [u8; 32] =
        current_script.args().raw_data()[0..32].try_into().unwrap();
    let inputs_type_hashes = QueryIter::new(load_cell_type_hash, Source::GroupInput);
    inputs_type_hashes.into_iter().all(|i| match i {
        Some(hash) => hash != global_registry_script_hash,
        None => true,
    })
}

fn exec_wrapped_script(current_script: &Script) -> Result<(), Error> {
    let global_registry_script_hash: [u8; 32] =
        current_script.args().raw_data()[0..32].try_into().unwrap();
    let args_key: [u8; 32] = current_script.args().raw_data()[32..64].try_into().unwrap();

    let cell_dep_type_hash = load_cell_type_hash(0, Source::CellDep)?;
    if cell_dep_type_hash
        .map(|h| h == global_registry_script_hash)
        .unwrap_or_default()
    {
        let cell_dep_lock_script = load_cell_lock(0, Source::CellDep)?;
        if cell_dep_lock_script.code_hash().as_bytes() != current_script.code_hash().as_bytes()
            || cell_dep_lock_script.hash_type() != current_script.hash_type()
            || cell_dep_lock_script.args().len() != 64
        {
            return Err(Error::InvalidCellDepRef);
        }

        let data = load_cell_data(0, Source::CellDep)?;
        if data.len() != 64 {
            return Err(Error::InvalidDataLength);
        }

        let start: [u8; 32] = cell_dep_lock_script.args().raw_data()[32..64]
            .try_into()
            .unwrap();
        match start.cmp(&args_key) {
            Ordering::Equal => {
                let args_value: [u8; 32] = data[32..64].try_into().unwrap();
                exec_wrapped_script_inner(args_value)
            }
            Ordering::Less => {
                let end: [u8; 32] = data[0..32].try_into().unwrap();
                if end >= args_key {
                    exec_wrapped_script_inner(args_key)
                } else {
                    return Err(Error::InvalidCellDepRef);
                }
            }
            Ordering::Greater => {
                return Err(Error::InvalidCellDepRef);
            }
        }
    } else {
        Err(Error::InvalidCellDepTypeScript)
    }
}

fn validate_config_value(current_script: &Script) -> Result<(), Error> {
    let global_registry_script_hash: [u8; 32] =
        current_script.args().raw_data()[0..32].try_into().unwrap();
    let inputs_type_hashes = QueryIter::new(load_cell_type_hash, Source::Input);

    let inputs_index: Vec<usize> = inputs_type_hashes
        .enumerate()
        .filter_map(|(index, i)| match i {
            Some(hash) => {
                if hash == global_registry_script_hash {
                    Some(index)
                } else {
                    None
                }
            }
            None => None,
        })
        .collect();

    if inputs_index.len() != 1 {
        return Err(Error::InvalidInputCount);
    }

    let index = inputs_index[0];

    let output = load_cell(index, Source::Output)?;
    if current_script.as_bytes() != output.lock().as_bytes() {
        return Err(Error::InvalidOutputLockScript);
    }

    let input_data = load_cell_data(index, Source::Input)?;
    let output_data = load_cell_data(index, Source::Output)?;
    if input_data[32..64] == output_data[32..64] {
        // if config value is not changed, skip validation
        return Ok(());
    } else {
        // else, verify by executing wrapped script
        exec_wrapped_script_inner(input_data[32..64].try_into().unwrap())
    }
}

fn exec_wrapped_script_inner(args: [u8; 32]) -> Result<(), Error> {
    // in this example, we just print the args
    // TODO: use syscall::exec or spawn to run wrapped script
    debug!("exec_wrapped_script_inner: {:?}", args);
    Ok(())
}
