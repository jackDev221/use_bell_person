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
use crate::serialize_job;



// #[tokio::test(flavor ="multi_thread", worker_threads = 10)]
// async fn do_create_proof() {
//     let (sender, mut receiver) = watch::channel("input".to_string()) ;
//     let assembly_queue = Buffer::new(4);
//     let shared_assembly_queue = Arc::new(Mutex::new(assembly_queue));
//     let producer = shared_assembly_queue.clone();
//     let consumer = shared_assembly_queue.clone();
//
//     let localhost_v4 = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
//     tokio::spawn(incoming_socket_listener(
//         localhost_v4,
//         8777,
//         producer,
//     ));
//
//     tokio::spawn(async move {
//         println!("Start run prover");
//         let reporter = ProverReporterTest::new(sender);
//         let synthesize = SynthesizedCircuitProviderTest::new(consumer);
//         run_prover_with_remote_synthesizer(
//             synthesize,
//             reporter,
//         );
//     });
//
//     tokio::spawn(async move {
//         println!("into send");
//         let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8777);
//         let mut input: Vec<u8> = vec![];
//         serialize_job(12, 15, &mut input);
//         tokio::time::sleep(Duration::from_secs(3)).await;
//         println!("start send:{:?}",input);
//         let res = send_assembly(
//             0,
//             &mut input,
//             &socket,
//         );
//         println!("send result {:?}", res);
//     });
//
//
//     while receiver.changed().await.is_ok() {
//         println!("received = {:?}", *receiver.borrow());
//         break;
//     }
//     exit(0);
// }
//
//
