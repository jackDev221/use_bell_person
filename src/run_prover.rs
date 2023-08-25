use std::sync::mpsc::{Receiver, Sender};
use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;
use prover_service::{JobReporter, JobResult, RemoteSynthesizer};
use prover_service::run_prover::ThreadGuard;
use zkevm_test_harness::abstract_zksync_circuit::concrete_circuits::ZkSyncProof;
use crate::prover::Prover;

struct ProverMessage(usize, Vec<u8>);

unsafe impl Send for ProverMessage {}

unsafe impl Sync for ProverMessage {}

const PULLING_DURATION_MILLIS: u64 = 1;

pub(crate) const PROVER_THREAD_HANDLE: u8 = 0;
pub(crate) const SETUP_THREAD_HANDLE: u8 = 1;
pub(crate) const PROOF_GENERATION_THREAD_HANDLE: u8 = 4;


pub(crate) struct ProverContext {
    prover_input_sender: Sender<ProverMessage>,
    prover_instance_receiver: Receiver<(usize, Prover, std::time::Instant)>,
    prover_instance_sender: Sender<(usize, Prover, std::time::Instant)>,
    prover_input_receiver: Receiver<ProverMessage>,
    pub(crate) thread_status_sender: Sender<u8>,
    thread_status_receiver: Receiver<u8>,
    pub(crate) report_sender: Sender<JobResult>,
    report_receiver: Receiver<JobResult>,
}

unsafe impl Send for ProverContext {}

unsafe impl Sync for ProverContext {}


impl ProverContext {
    fn init() -> Self {
        let (prover_input_sender, prover_input_receiver) = channel();
        let (prover_instance_sender, prover_instance_receiver) = channel();
        let (report_sender, report_receiver) = channel();
        let (thread_status_sender, thread_status_receiver) = channel();
        let provers = create_prover_instances();
        for (prover_idx, prover) in provers {
            let prover_instance = (prover_idx, prover, std::time::Instant::now());
            prover_instance_sender.send(prover_instance).unwrap();
        }
        Self {
            prover_input_sender,
            prover_input_receiver,
            prover_instance_sender,
            prover_instance_receiver,
            thread_status_sender,
            thread_status_receiver,
            report_sender,
            report_receiver,
        }
    }
}

pub(crate) fn create_prover_instances() -> Vec<(usize, Prover)> {
    vec![(0, Prover::new())]
}

pub fn run_prover_with_remote_synthesizer<
    RS: RemoteSynthesizer + 'static,
    JR: JobReporter + 'static,
>(
    mut remote_synthesizer: RS,
    job_reporter: JR,
) {
    let ctx = ProverContext::init();
    let ctx = Arc::new(ctx);
    thread_liveness_tracker(ctx.clone(), job_reporter);
    main_prover_handler(ctx.clone());
    let mut scheduler_is_idle = std::time::Instant::now();
    let duration = Duration::from_millis(PULLING_DURATION_MILLIS);
    loop {
        if let Some(mut encoded_assembly) = remote_synthesizer.try_next() {
            let scheduler_received_input = scheduler_is_idle.elapsed();
            ctx.report_sender
                .send(JobResult::SchedulerWaitedIdle(scheduler_received_input))
                .unwrap();
            scheduler_is_idle = std::time::Instant::now();
            let mut job_id_bytes = [0u8; 8];
            encoded_assembly.read_exact(&mut job_id_bytes[..]).unwrap();
            let job_id = usize::from_le_bytes(job_id_bytes);
            let mut input: Vec<u8> = vec![];
            let _ = encoded_assembly.read_to_end(&mut input);
            println!("proof input {:?}", input);
            ctx.prover_input_sender.send(ProverMessage(job_id, input)).unwrap();
        } else {
            sleep_for_duration(duration);
        }
    }
}

fn sleep_for_duration(duration: Duration) {
    std::thread::sleep(duration);
}

fn thread_liveness_tracker<JR: JobReporter + 'static, >(
    ctx: Arc<ProverContext>,
    mut job_reporter: JR,
) {
    let duration = Duration::from_millis(PULLING_DURATION_MILLIS);
    std::thread::spawn(move || loop {
        for report in ctx.report_receiver.try_iter() {
            job_reporter.send_report(report)
        }
        for thread_id in ctx.thread_status_receiver.try_iter() {
            match thread_id {
                SETUP_THREAD_HANDLE => {
                    println!("re-spawning setup loading thread");
                    // setup_loader(ctx.clone(), artifact_manager.clone(), params.clone());
                }
                PROVER_THREAD_HANDLE => {
                    println!("re-spawning prover thread");
                    main_prover_handler(ctx.clone())
                }
                _ => (),
            }
        }
        sleep_for_duration(duration);
    });
}

fn main_prover_handler(ctx: Arc<ProverContext>) {
    let duration = Duration::from_millis(PULLING_DURATION_MILLIS);
    std::thread::spawn(move || {
        loop {
            if let Ok((prover_idx, prover, prover_instance_become_idle)) = ctx.prover_instance_receiver.try_recv() {
                let input = ctx.prover_input_receiver.recv().unwrap();
                let prover = (prover_idx.clone(), prover);
                let prover_input_receiver = prover_instance_become_idle.elapsed();
                ctx.report_sender.send(
                    JobResult::ProverWaitedIdle(
                        prover_idx,
                        prover_input_receiver,
                    )
                ).unwrap();

                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    create_proof(ctx, input, prover)
                });
            } else {
                sleep_for_duration(duration);
            }
        }
    });
}

fn create_proof(
    ctx: Arc<ProverContext>,
    input: ProverMessage,
    prover: (usize, Prover),
) {
    println!("Start create proof for id :{}", prover.0);
    let job_id = input.0;
    let _guard = ThreadGuard::new(
        PROOF_GENERATION_THREAD_HANDLE,
        job_id.clone(),
        ctx.report_sender.clone(),
        ctx.thread_status_sender.clone(),
    );

    let (prover_idx, mut prover) = prover;
    let proof_generated = std::time::Instant::now();
    let report = match prover.create_proof(input.0.clone(), &input.1) {
        Ok(proof) => {
            println!("Finish create proof for id :{}", job_id);
            let proof_generated = proof_generated.elapsed();
            let proof = ZkSyncProof::from_proof_and_numeric_type(0, proof);
            JobResult::ProofGenerated(job_id.clone(), proof_generated, proof, prover_idx.clone())
        }
        Err(err) => {
            println!("Fail to create proof for id :{}, err:{:?}", job_id, err);
            JobResult::Failure(job_id.clone(), format!("{} proof verification failed", prover_idx).clone())
        }
    };
    ctx.prover_instance_sender.send((prover_idx, prover, std::time::Instant::now())).unwrap();
    ctx.report_sender.send(report).unwrap();
}
