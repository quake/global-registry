use super::*;
use ckb_testtool::builtin::ALWAYS_SUCCESS;
use ckb_testtool::ckb_error::Error;
use ckb_testtool::ckb_types::{bytes::Bytes, core::TransactionBuilder, packed::*, prelude::*};
use ckb_testtool::context::{random_hash, Context};

const MAX_CYCLES: u64 = 10_000_000;

// error numbers
const ERROR_EMPTY_ARGS: i8 = 5;

fn assert_script_error(err: Error, err_code: i8) {
    let error_string = err.to_string();
    assert!(
        error_string.contains(format!("error code {} ", err_code).as_str()),
        "error_string: {}, expected_error_code: {}",
        error_string,
        err_code
    );
}

#[test]
fn test_init_global_registry() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("global-registry");
    let gr_out_point = context.deploy_cell(contract_bin);
    let as_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    // prepare lock script
    let lock_script = context
        .build_script(&as_out_point, Bytes::new())
        .expect("script");

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(2000u64.pack())
            .lock(lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();

    // prepare type script
    let mut hash = [0; 32];
    let mut blake2b = blake2b_rs::Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build();
    blake2b.update(input.as_slice());
    blake2b.update(&0u64.to_le_bytes());
    blake2b.finalize(&mut hash);

    let type_script = context
        .build_script(&gr_out_point, Bytes::from(hash.to_vec()))
        .expect("script");

    // prepare outputs
    let output_lock_script = context
        .build_script(&as_out_point, Bytes::from(vec![0u8; 32]))
        .expect("script");

    let outputs = vec![
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(output_lock_script)
            .type_(ScriptOpt::new_builder().set(Some(type_script)).build())
            .build(),
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::from(vec![255u8; 32]), Bytes::new()];

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_update_global_registry() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("global-registry");
    let gr_out_point = context.deploy_cell(contract_bin);
    let as_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    // prepare lock script and type script
    let lock_script = context
        .build_script(&as_out_point, Bytes::from(vec![0u8; 32]))
        .expect("script");

    let type_script = ScriptOpt::new_builder()
        .set(Some(
            context
                .build_script(&gr_out_point, random_hash().as_bytes())
                .expect("script"),
        ))
        .build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(3000u64.pack())
            .lock(lock_script.clone())
            .type_(type_script.clone())
            .build(),
        Bytes::from(vec![255u8; 32]),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();

    // prepare outputs
    let output_lock_script_1 = context
        .build_script(&as_out_point, Bytes::from(vec![0u8; 32]))
        .expect("script");

    let output_lock_script_2 = context
        .build_script(&as_out_point, Bytes::from(vec![100u8; 32]))
        .expect("script");

    let output_lock_script_3 = context
        .build_script(&as_out_point, Bytes::from(vec![200u8; 32]))
        .expect("script");

    let outputs = vec![
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(output_lock_script_1)
            .type_(type_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(output_lock_script_2)
            .type_(type_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(output_lock_script_3)
            .type_(type_script.clone())
            .build(),
    ];

    let outputs_data = vec![
        Bytes::from(vec![100u8; 32]),
        Bytes::from(vec![200u8; 32]),
        Bytes::from(vec![255u8; 32]),
    ];

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}
