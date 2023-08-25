use std::io;
use zkevm_test_harness::bellman::plonk::better_better_cs::cs::Circuit;
use zkevm_test_harness::bellman::bn256::Bn256;
use zkevm_test_harness::bellman::plonk::better_better_cs::proof::Proof;

use crate::use_bellperson::gen_and_vk_proof;
use rand::{thread_rng, Rng};

pub struct Prover {}


impl Prover {
    pub fn new() -> Self { Self {} }
    pub fn create_proof<C: Circuit<Bn256>, >(&mut self, times: usize, input: &Vec<u8>) -> Result<Proof<Bn256, C>, io::Error> {
        println!("create_proof start, hash times: {} ", times);
        let mut times = 0;
        loop {
            times += 1;
            let mut data = vec![];
            let mut rng = thread_rng();
            data.extend_from_slice(input);
            while data.len() < times {
                let rand_bytes: [u8; 32] = rng.gen();
                if data.len() + 32 < 160 {
                    data.extend_from_slice(rand_bytes.as_slice());
                } else {
                    let lasts = rand_bytes[0..(160 - data.len())].to_vec();
                    data.extend_from_slice(lasts.as_slice());
                }
            }
            let preiamge: [u8; 160] = data.as_slice().try_into().expect("ddd");
            let hash = gen_and_vk_proof(preiamge);
            if (hash[0] == 0 && hash[1] == 0 && hash[2] == 0) || times > 66 {
                break;
            }
        }
        println!("finish_proof");

        Ok(Proof::empty())
    }
}
