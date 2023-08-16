use std::io;
use zkevm_test_harness::bellman::plonk::better_better_cs::cs::Circuit;
use zkevm_test_harness::bellman::bn256::Bn256;
use zkevm_test_harness::bellman::plonk::better_better_cs::proof::Proof;

use crate::use_bellperson::gen_and_vk_proof;

pub struct Prover {}


impl Prover {
    pub fn new() -> Self { Self {} }
    pub fn create_proof<C: Circuit<Bn256>, >(&mut self, input: &Vec<u8>) -> Result<Proof<Bn256, C>, io::Error> {
        println!("create_proof:{:?} ", input);
        gen_and_vk_proof();
        Ok(Proof::empty())
    }
}
