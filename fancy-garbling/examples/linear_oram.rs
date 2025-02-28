//! An example that secretly retrieves an element from an ORAM in a binary garbled circuit
//! using fancy-garbling.
use fancy_garbling::{
    twopac::semihonest::{Evaluator, Garbler},
    util, AllWire, BinaryBundle, BinaryGadgets, Fancy, FancyArithmetic, FancyBinary, FancyInput,
    FancyReveal,
};

use ocelot::{ot::AlszReceiver as OtReceiver, ot::AlszSender as OtSender};
use scuttlebutt::{AbstractChannel, AesRng, Channel};

use std::fmt::Debug;
use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
};

/// A structure that contains both the garbler and the evaluators
/// wires. This structure simplifies the API of the garbled circuit.
struct ORAMInputs<F> {
    ram: Vec<BinaryBundle<F>>,
    query: BinaryBundle<F>,
}
/// The garbler's main method:
/// (1) The garbler is first created using the passed rng and value.
/// (2) The garbler then exchanges their wires obliviously with the evaluator.
/// (3) The garbler and the evaluator then run the garbled circuit.
/// (4) The garbler and the evaluator open the result of the computation.
fn gb_linear_oram<C>(rng: &mut AesRng, channel: &mut C, inputs: &[u128])
where
    C: AbstractChannel + std::clone::Clone,
{
    // (1)
    let mut gb =
        Garbler::<C, AesRng, OtSender, AllWire>::new(channel.clone(), rng.clone()).unwrap();
    // The size of the RAM is assumed to be public. The garbler sends their number of
    // of input wires. We note that every element of the RAM has a fixed size of 128 bits.
    let _ = channel.write_usize(inputs.len());
    // (2)
    let circuit_wires = gb_set_fancy_inputs(&mut gb, inputs);
    // (3)
    let query =
        fancy_linear_oram::<Garbler<C, AesRng, OtSender, AllWire>>(&mut gb, circuit_wires).unwrap();
    // (4)
    gb.outputs(query.wires()).unwrap();
}

/// The garbler's wire exchange method
fn gb_set_fancy_inputs<F, E>(gb: &mut F, inputs: &[u128]) -> ORAMInputs<F::Item>
where
    F: FancyInput<Item = AllWire, Error = E>,
    E: Debug,
{
    // The number of bits needed to represent a single input value
    let nbits = 128;
    // The garbler encodes their wires with the appropriate moduli per wire.
    let ram: Vec<BinaryBundle<F::Item>> = gb.bin_encode_many(inputs, nbits).unwrap();
    // The evaluator receives their input labels using Oblivious Transfer (OT)
    let query: BinaryBundle<F::Item> = gb.bin_receive(nbits).unwrap();

    ORAMInputs { ram, query }
}

/// The evaluator's main method:
/// (1) The evaluator is first created using the passed rng and value.
/// (2) The evaluator then exchanges their wires obliviously with the garbler.
/// (3) The evaluator and the garbler then run the garbled circuit.
/// (4) The evaluator and the garbler open the result of the computation.
/// (5) The evaluator translates the binary output of the circuit into its decimal
///     representation.

fn ev_linear_oram<C>(rng: &mut AesRng, channel: &mut C, input: u128) -> u128
where
    C: AbstractChannel + std::clone::Clone,
{
    // (1)
    let mut ev =
        Evaluator::<C, AesRng, OtReceiver, AllWire>::new(channel.clone(), rng.clone()).unwrap();
    let ram_size = channel.read_usize().unwrap();
    // (2)
    let circuit_wires = ev_set_fancy_inputs(&mut ev, input, ram_size);
    // (3)
    let query =
        fancy_linear_oram::<Evaluator<C, AesRng, OtReceiver, AllWire>>(&mut ev, circuit_wires)
            .unwrap();
    // (4)
    let query_binary = ev
        .outputs(query.wires())
        .unwrap()
        .expect("evaluator should produce outputs");

    // (5)
    util::u128_from_bits(&query_binary)
}
fn ev_set_fancy_inputs<F, E>(ev: &mut F, input: u128, ram_size: usize) -> ORAMInputs<F::Item>
where
    F: FancyInput<Item = AllWire, Error = E>,
    E: Debug,
{
    // The number of bits needed to represent a single input value
    let nbits = 128;
    // The evaluator receives the garblers input labels.
    let ram: Vec<BinaryBundle<F::Item>> = ev.bin_receive_many(ram_size, nbits).unwrap();
    // The evaluator encodes their input labels.
    let query: BinaryBundle<F::Item> = ev.bin_encode(input, nbits).unwrap();

    ORAMInputs { ram, query }
}

/// The main fancy function which describes the garbled circuit for linear ORAM.
fn fancy_linear_oram<F>(
    f: &mut F,
    wire_inputs: ORAMInputs<F::Item>,
) -> Result<BinaryBundle<F::Item>, F::Error>
where
    F: FancyReveal + Fancy + BinaryGadgets + FancyBinary + FancyArithmetic,
{
    let ram: Vec<BinaryBundle<_>> = wire_inputs.ram;
    let index: BinaryBundle<_> = wire_inputs.query;

    let mut result = f.bin_constant_bundle(0, 128)?;
    let zero = f.bin_constant_bundle(0, 128)?;

    // We traverse the garbler's RAM one element at a time, and multiplex
    // the result based on whether the evaluator's query matches the current
    // index.
    for (i, item) in ram.iter().enumerate() {
        // The current index is turned into a binary constant bundle.
        let current_index = f.bin_constant_bundle(i as u128, 128)?;
        // We check if the evaluator's query matches the current index obliviously.
        let mux_bit = f.bin_eq_bundles(&index, &current_index)?;
        // We use the result of the prior equality check to multiplex by either adding 0 to
        // the result of the computation and keeping it as is, or adding RAM[i] to it
        // and updating it. The evaluator's query can only correspond to a single index.
        let mux = f.bin_multiplex(&mux_bit, &zero, item)?;
        result = f.bin_addition_no_carry(&result, &mux)?;
    }

    Ok(result)
}

fn ram_in_clear(index: usize, ram: &[u128]) -> u128 {
    ram[index]
}

use clap::Parser;
#[derive(Parser)]
/// Example usage:
///
/// cargo run --example linear_oram 5 1 2 3 7 7 25
///
/// Computes RAM([1,2,3,7,7,25], at index: 5)
struct Cli {
    /// The first integer specifies the evaluator's query
    query: u128,
    /// The rest of the integers contitute the garbler's RAM
    ram: Vec<u128>,
}

fn main() {
    let cli = Cli::parse();

    let ev_index: u128 = cli.query;
    let gb_ram = cli.ram;

    let (sender, receiver) = UnixStream::pair().unwrap();
    std::thread::scope(|s| {
        s.spawn(|| {
            let mut rng_gb = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);
            gb_linear_oram(&mut rng_gb, &mut channel, &gb_ram);
        });
        let rng_ev = AesRng::new();
        let reader = BufReader::new(receiver.try_clone().unwrap());
        let writer = BufWriter::new(receiver);
        let mut channel = Channel::new(reader, writer);

        let result = ev_linear_oram(&mut rng_ev.clone(), &mut channel, ev_index);
        let resut_in_clear = ram_in_clear(ev_index as usize, &gb_ram);
        println!(
            "Garbled Circuit result is : RAM([{:?}], at index:{}) = {}",
            gb_ram, ev_index, result
        );
        assert!(
            result == resut_in_clear,
            "The result is incorrect and should be {}",
            resut_in_clear
        );
    });
}
