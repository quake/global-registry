// Import from `core` instead of from `std` since we are in no-std mode
use core::{cmp::Ordering, result::Result};

// Import heap related library from `alloc`
// https://doc.rust-lang.org/alloc/index.html
use alloc::vec::Vec;

// Import CKB syscalls and structures
// https://docs.rs/ckb-std/
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{
        core::ScriptHashType,
        packed::{Script, ScriptReader},
        prelude::*,
    },
    debug,
    high_level::{
        encode_hex, exec_cell, load_cell, load_cell_data, load_cell_lock, load_cell_type_hash,
        load_script, load_witness, QueryIter,
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
    let wrapped_script_hash: [u8; 32] =
        current_script.args().raw_data()[32..64].try_into().unwrap();

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
        match start.cmp(&wrapped_script_hash) {
            Ordering::Equal => {
                let config_wrapped_script_hash: [u8; 32] = data[32..64].try_into().unwrap();
                exec_wrapped_script_inner(config_wrapped_script_hash)
            }
            Ordering::Less => {
                let end: [u8; 32] = data[0..32].try_into().unwrap();
                if end >= wrapped_script_hash {
                    exec_wrapped_script_inner(wrapped_script_hash)
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

fn exec_wrapped_script_inner(wrapped_script_hash: [u8; 32]) -> Result<(), Error> {
    let witness = load_witness(0, Source::GroupInput)?;
    let (wrapped_script, wrapped_script_witness_index) = parse_witness(&witness)?;
    let script_hash = calc_script_hash(&wrapped_script);
    if script_hash != wrapped_script_hash {
        return Err(Error::InvalidWrappedScriptHash);
    }

    let hash_type = if wrapped_script.hash_type().as_slice() == &[1] {
        ScriptHashType::Type
    } else {
        ScriptHashType::Data
    };

    let arg0 = encode_hex(&wrapped_script.args().raw_data().to_vec());
    let arg1 = encode_hex(&wrapped_script_witness_index.to_le_bytes());
    debug!("arg0: {:?}", arg0);
    debug!("arg1: {:?}", arg1);

    exec_cell(
        wrapped_script.code_hash().as_slice(),
        hash_type,
        &[&arg0, &arg1],
    )?;
    Ok(())
}

fn parse_witness(witness: &[u8]) -> Result<(Script, u16), Error> {
    // 2 (witness_index) + 53 (min script size)
    if witness.len() < 55 {
        return Err(Error::InvalidWitnessFormat);
    }
    let wrapped_script_witness_index = u16::from_le_bytes(witness[0..2].try_into().unwrap());
    let wrapped_script_data = witness[2..].to_vec();
    match ScriptReader::verify(&wrapped_script_data, false) {
        Ok(()) => {
            let wrapped_script = Script::new_unchecked(wrapped_script_data.into());
            Ok((wrapped_script, wrapped_script_witness_index))
        }
        Err(_err) => Err(Error::InvalidWitnessFormat),
    }
}

fn calc_script_hash(script: &Script) -> [u8; 32] {
    let mut hash = [0; 32];
    let mut blake2b = blake2b_rs::Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build();
    blake2b.update(script.as_slice());
    blake2b.finalize(&mut hash);
    hash
}
