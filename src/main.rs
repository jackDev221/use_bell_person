use std::io::Write;

pub mod run_prover;
pub mod prover;
#[cfg(test)]
mod tests;
mod use_bellperson;

use std::sync::{Arc, Mutex};
use queues::{Buffer, IsQueue};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::io::{copy as std_copy, ErrorKind};
use std::io::Cursor;
use std::io::Read;
use zkevm_test_harness::bellman::bn256::Bn256;
use prover_service::{JobReporter, JobResult, RemoteSynthesizer};
use prover_service::JobResult::{Failure, ProofGenerated};
use tokio::{io::copy, net::{TcpListener, TcpStream}};
use std::net::TcpStream as StdTcpStream;
use std::process::exit;
use zkevm_test_harness::abstract_zksync_circuit::concrete_circuits::ZkSyncProof;
use crate::run_prover::run_prover_with_remote_synthesizer;
use tokio::sync::watch;


pub fn serialize_job<W: Write>(
    job_id: usize,
    batch_number: u32,
    buffer: &mut W,
) {
    buffer.write_all(&job_id.to_le_bytes()).unwrap();
    buffer.write_all(&batch_number.to_le_bytes()).unwrap();
    let address = hex::decode("b933a7d3ce9dade7e138abcc7c08ceeaf78e99a2").unwrap();
    buffer.write_all(address.as_slice()).unwrap();
}

pub type SharedAssemblyQueue = Arc<Mutex<Buffer<Vec<u8>>>>;


struct SynthesizedCircuitProviderTest {
    queue: SharedAssemblyQueue,
}

impl SynthesizedCircuitProviderTest {
    pub fn new(
        queue: SharedAssemblyQueue,
    ) -> Self {
        Self {
            queue,
        }
    }
}

impl RemoteSynthesizer for SynthesizedCircuitProviderTest {
    fn try_next(&mut self) -> Option<Box<dyn Read + Send + Sync>> {
        let mut assembly_queue = self.queue.lock().unwrap();
        return match assembly_queue.remove() {
            Ok(blob) => {
                let queue_free_slots = assembly_queue.capacity() - assembly_queue.size();
                println!(
                    "Queue free slot {} for capacity {}",
                    queue_free_slots,
                    assembly_queue.capacity()
                );

                Some(Box::new(Cursor::new(blob)))
            }
            Err(_) => None,
        };
    }
}


#[allow(clippy::too_many_arguments)]
pub async fn incoming_socket_listener(
    host: IpAddr,
    port: u16,
    queue: SharedAssemblyQueue,
) {
    let listening_address = SocketAddr::new(host, port);
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!(
        "Starting assembly receiver at host: {}, port: {}",
        host,
        port
    );
    let listener = TcpListener::bind(listening_address)
        .await
        .unwrap_or_else(|_| panic!("Failed binding address: {:?}", listening_address));
    let mut now = Instant::now();
    loop {
        let stream = match listener.accept().await {
            Ok(stream) => stream.0,
            Err(e) => {
                panic!("could not accept connection: {e:?}");
            }
        };
        println!(
            "Received new assembly send connection, waited for {}ms.",
            now.elapsed().as_millis()
        );

        handle_incoming_file(
            stream,
            queue.clone(),
        ).await;

        now = Instant::now();
    }
}

async fn handle_incoming_file(
    mut stream: TcpStream,
    queue: SharedAssemblyQueue,
) {
    let mut assembly: Vec<u8> = vec![];
    let started_at = Instant::now();
    copy(&mut stream, &mut assembly).await.expect("Failed reading from stream");
    let file_size_in_gb = assembly.len();
    println!(
        "Read file of size: {} from stream took: {} seconds",
        file_size_in_gb,
        started_at.elapsed().as_secs()
    );
    let mut assembly_queue = queue.lock().unwrap();
    assembly_queue
        .add(assembly)
        .expect("Failed saving assembly to queue");
}

#[derive(Debug)]
struct ProverReporterTest {
    pub sender: watch::Sender<String>,
}

impl ProverReporterTest {
    pub(crate) fn new(
        sender: watch::Sender<String>
    ) -> Self {
        Self {
            sender: sender
        }
    }

    fn handle_successful_proof_generation(
        &self,
        job_id: usize,
        proof: ZkSyncProof<Bn256>,
        duration: Duration,
        index: usize,
    ) {
        let serialized = bincode::serialize(&proof).expect("Failed to serialize proof");
        println!(
            "Successfully generated proof with id {:?} for index: {}. Size: {:?}KB took: {:?}",
            job_id,
            index,
            serialized.len() >> 10,
            duration,
        );
    }
}

impl JobReporter for ProverReporterTest {
    fn send_report(&mut self, report: JobResult) {
        let res = match report {
            Failure(job_id, error) => {
                println!(
                    "Failed to generate proof for id {:?}. error reason; {}",
                    job_id,
                    error
                );
                format!("Failed to generate proof for id {:?}. error reason {}", job_id, error)
            }
            ProofGenerated(job_id, duration, proof, index) => {
                self.handle_successful_proof_generation(job_id, proof, duration, index);
                format!("Finish to generate proof for id {:?}", job_id)
                // let _ = self.sender.send();
            }

            JobResult::Synthesized(job_id, duration) => {
                println!(
                    "Successfully synthesized circuit with id {:?} . took: {:?}",
                    job_id,
                    duration,
                );
                format!("")
            }

            JobResult::AssemblyFinalized(job_id, duration) => {
                println!(
                    "Successfully finalized assembly with id {:?} d. took: {:?}",
                    job_id,
                    duration,
                );
                format!("")
            }

            JobResult::SetupLoaded(job_id, duration, cache_miss) => {
                println!(
                    "Successfully setup loaded with id {:?} . \
                     took: {:?} and had cache_miss: {}",
                    job_id,
                    duration,
                    cache_miss
                );
                format!("")
            }
            JobResult::AssemblyEncoded(job_id, duration) => {
                println!(
                    "Successfully encoded assembly with id {:?}. took: {:?}",
                    job_id,
                    duration,
                );
                format!("")
            }
            JobResult::AssemblyDecoded(job_id, duration) => {
                println!(
                    "Successfully decoded assembly with id {:?} . took: {:?}",
                    job_id,
                    duration,
                );
                format!("")
            }
            JobResult::FailureWithDebugging(job_id, circuit_id, _assembly, error) => {
                println!(
                    "Failed assembly decoding for job-id {} and circuit-type: {}. error: {}",
                    job_id,
                    circuit_id,
                    error,
                );
                format!("")
            }
            JobResult::AssemblyTransferred(job_id, duration) => {
                println!(
                    "Successfully transferred assembly with id {:?} . took: {:?}",
                    job_id,
                    duration,
                );
                format!("")
            }
            JobResult::ProverWaitedIdle(prover_id, duration) => {
                println!(
                    "Prover wait idle time: {:?} for prover-id: {:?}",
                    duration,
                    prover_id
                );
                format!("")
            }
            JobResult::SetupLoaderWaitedIdle(duration) => {
                println!("Setup load wait idle time: {:?}", duration);
                format!("")
            }
            JobResult::SchedulerWaitedIdle(duration) => {
                println!("Scheduler wait idle time: {:?}", duration);
                format!("")
            }
        };
        if res.len() > 0 {
            // let sender_op = self.sender;
            let _ = self.sender.send(res);
        }
    }
}

fn send(read: &mut impl Read, tcp: &mut StdTcpStream) -> std::io::Result<u64> {
    let mut attempts = 10;
    let mut last_result = Ok(0);

    while attempts > 0 {
        match std_copy(read, tcp) {
            Ok(copied) => return Ok(copied),
            Err(err) if can_be_retried(err.kind()) => {
                attempts -= 1;
                last_result = Err(err);
            }
            Err(err) => return Err(err),
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    last_result
}

fn can_be_retried(err: ErrorKind) -> bool {
    matches!(err, ErrorKind::TimedOut | ErrorKind::ConnectionRefused)
}

fn send_assembly(
    _job_id: u32,
    serialized: &mut Vec<u8>,
    socket_address: &SocketAddr,
) -> Result<(Duration, u64), String> {
    let started_at = std::time::Instant::now();
    match StdTcpStream::connect(socket_address) {
        Ok(mut stream) => {
            return send(&mut serialized.as_slice(), &mut stream)
                .map(|result| (started_at.elapsed(), result))
                .map_err(|err| format!("Could not send assembly to prover: {err:?}"));
        }
        Err(err) => {
            return Err(format!("{err:?}"));
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let number = args[1].parse::<u32>().unwrap();
    let (sender, mut receiver) = watch::channel("input".to_string());
    let assembly_queue = Buffer::new(4);
    let shared_assembly_queue = Arc::new(Mutex::new(assembly_queue));
    let producer = shared_assembly_queue.clone();
    let consumer = shared_assembly_queue.clone();

    let localhost_v4 = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
    tokio::spawn(incoming_socket_listener(
        localhost_v4,
        8777,
        producer,
    ));

    tokio::spawn(async move {
        println!("Start run prover");
        let reporter = ProverReporterTest::new(sender);
        let synthesize = SynthesizedCircuitProviderTest::new(consumer);
        run_prover_with_remote_synthesizer(
            synthesize,
            reporter,
        );
    });
    let id = number.clone() as usize;
    tokio::spawn(async move {
        println!("into send");
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8777);
        let mut input: Vec<u8> = vec![];
        serialize_job(id, number, &mut input);
        tokio::time::sleep(Duration::from_secs(3)).await;
        println!("start send:{:?}", input);
        let res = send_assembly(
            0,
            &mut input,
            &socket,
        );
        println!("send result {:?}", res);
    });


    while receiver.changed().await.is_ok() {
        println!("received = {:?}", *receiver.borrow());
        break;
    }
    exit(0);
}
