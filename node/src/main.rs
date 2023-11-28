use anyhow::{Result};
use clap::{App, AppSettings, ArgMatches, SubCommand};
use env_logger::Env;
use log::info;
use tokio::sync::mpsc::{channel, Receiver};

use consensus::Consensus;
use model::block::Block;
use model::committee::{Committee, Id};
use model::vertex::Vertex;
use transaction::TransactionCoordinator;
use vertex::vertex_coordinator::VertexCoordinator;

use std::fs::File;
use std::io::Write;
use serde_json;

pub const DEFAULT_CHANNEL_CAPACITY: usize = 1000;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("ABSE-DAG-Rider")
        .version("1.0")
        .about("ABSE-DAG-Rider")
        .subcommand(
            SubCommand::with_name("run")
                .about("Run a node")
                .args_from_usage("--id=<INT> 'Node id'")
                .args_from_usage("--pretend_failure=<PF> 'pretend to be a faulty node'")
                .args_from_usage("--committee=<PATH> 'Path to committee JSON file'")
                .args_from_usage("--channel_capacity=[CAPACITY] 'Channel capacity'")
                .args_from_usage("--batch_size=[SIZE] 'Batch size'")
        )
        .subcommand(
          SubCommand::with_name("generate")
              .about("Generate committee and run nodes")
              .args_from_usage("--node_count=[COUNT] 'Number of nodes'")
              .args_from_usage("--faulty_count=[FCOUNT] 'Number of faulties'")
              .args_from_usage("--faulty_type=[FTYPE] 'Type of faulties, 1 represents simulating regular adversaries, 2 represents simulating special adversaries (as described in the paper)'")
              .args_from_usage("--channel_capacity=[CAPACITY] 'Channel capacity'")
              .args_from_usage("--batch_size=[SIZE] 'Batch size'")
        )
        .get_matches();

    let mut logger = env_logger::Builder::from_env(Env::default().default_filter_or("info"));
    logger.format_timestamp_millis().init();

    match matches.subcommand() {
        ("run", Some(sub_matches)) => run(sub_matches).await?,
        ("generate", Some(sub_matches)) => generate(sub_matches).await?,
        _ => unreachable!(),
    }
    Ok(())
}

async fn run(matches: &ArgMatches<'_>) -> Result<()> {
    let node_id = matches.value_of("id").unwrap().parse::<Id>().unwrap();

    let pretend_failure = matches
    .value_of("pretend_failure")
    .unwrap_or("0")
    .parse::<usize>()
    .unwrap();
    let mut ftype = 0;
    if pretend_failure == 0 {
      ftype = 0;
    }else if pretend_failure == 1 {
      ftype = 1;
    }else if pretend_failure == 2 {
      ftype = 2;
    }

    let mut pretend_failure = pretend_failure != 0;

    if ftype == 2 {
      pretend_failure = false;
    }
    

    let channel_capacity = matches
    .value_of("channel_capacity")
    .unwrap_or("1000")
    .parse::<usize>()
    .unwrap();

    let batch_size = matches
    .value_of("batch_size")
    .unwrap_or("10")
    .parse::<usize>()
    .unwrap();
    let committee_file = matches.value_of("committee").unwrap();

    // Load the committee from the file.
    let committee: Committee = serde_json::from_reader(File::open(committee_file)?)?;

    let (vertex_output_sender, vertex_output_receiver) = channel::<Vertex>(channel_capacity);

    let (vertex_to_broadcast_sender, vertex_to_broadcast_receiver) = channel::<Vertex>(channel_capacity);
    let (vertex_to_consensus_sender, vertex_to_consensus_receiver) = channel::<Vertex>(channel_capacity);
    let (block_sender, block_receiver) = channel::<Block>(channel_capacity);

    VertexCoordinator::spawn(
        node_id,
        //Committee::default(),
        committee.clone(),
        vertex_to_consensus_sender,
        vertex_to_broadcast_receiver,
        pretend_failure
    );

    TransactionCoordinator::spawn(
        node_id,
        //Committee::default(),
        committee.clone(),
        block_sender,
        batch_size,
        pretend_failure
    );

    Consensus::spawn(
        node_id,
        //Committee::default(),
        committee.clone(),
        vertex_to_consensus_receiver,
        vertex_to_broadcast_sender,
        vertex_output_sender,
        block_receiver,
        ftype
    );

    wait_and_print_vertexs(vertex_output_receiver).await;
    unreachable!();
}

async fn generate(matches: &ArgMatches<'_>) -> Result<()> {
  
  let node_count = matches
        .value_of("node_count")
        .unwrap_or("4")
        .parse::<usize>()
        .unwrap();

  let faulty_count = matches
        .value_of("faulty_count")
        .unwrap_or("0")
        .parse::<usize>()
        .unwrap(); 

  let faulty_type = matches
        .value_of("faulty_type")
        .unwrap_or("0")
        .parse::<usize>()
        .unwrap();       

  let mut ftype = 1;
  if faulty_type == 2{
    ftype = 2;
  }       
  
  if faulty_count > node_count - node_count / 3 * 2 - 1{
      println!("The number of malicious nodes is too high to meet 
      the minimum requirements for reaching consensus, at which point the throughput is 0.");
      return Ok(());
  }      

  let channel_capacity = matches
  .value_of("channel_capacity")
  .unwrap_or("1000")
  .parse::<usize>()
  .unwrap();

  let batch_size = matches
  .value_of("batch_size")
  .unwrap_or("10")
  .parse::<usize>()
  .unwrap();

  // Generate the committee.
  let committee = Committee::generate(node_count as u32);

  // Save the committee to a JSON file.
  let file = File::create("committee.json")?;
  serde_json::to_writer(file, &committee)?;

  // Generate a bash script to run the nodes.
  let mut script = File::create("run_nodes.sh")?;
  writeln!(script, "#!/bin/bash")?;
  for id in 1..=node_count {
    if id==1{
      if id > node_count - faulty_count{
        writeln!(script, "./node run --id={} --committee=committee.json --batch_size={} --channel_capacity={} --pretend_failure={} &", id, batch_size, channel_capacity, ftype)?;
      }else{
        writeln!(script, "./node run --id={} --committee=committee.json --batch_size={} --channel_capacity={} --pretend_failure=0 &", id, batch_size, channel_capacity)?;
      }
    }else{
      if id > node_count - faulty_count{
        writeln!(script, "./node run --id={} --committee=committee.json --batch_size={} --channel_capacity={} --pretend_failure={} &>/dev/null &", id, batch_size, channel_capacity, ftype)?;
      }else{
        writeln!(script, "./node run --id={} --committee=committee.json --batch_size={} --channel_capacity={} --pretend_failure=0 &>/dev/null &", id, batch_size, channel_capacity)?;
      }
      //writeln!(script, "./node run --id={} --committee=committee.json --batch_size={} --channel_capacity={} &>/dev/null &", id, batch_size, channel_capacity)?;
    }
    writeln!(script, "THREAD_{}=$!", id-1)?;
  }

  write!(script, "trap 'kill")?;

  for id in 0..=node_count-1 {
    write!(script, " $THREAD_{}", id)?;
  }

  writeln!(script, "' SIGINT SIGTERM")?;
  writeln!(script, "wait $THREAD_0")?;
  writeln!(script, "sleep 2")?;
  writeln!(script, "pkill -P $$")?;

  Ok(())
}

async fn wait_and_print_vertexs(mut vertex_output_receiver: Receiver<Vertex>) {
    while let Some(vertex) = vertex_output_receiver.recv().await {
        info!("Vertex committed: {}", vertex)
    }
}

// async fn default_run(matches: &ArgMatches<'_>) -> Result<()> {
//   let node_id = matches.value_of("id").unwrap().parse::<Id>().unwrap();

//   let channel_capacity = matches
//   .value_of("channel_capacity")
//   .unwrap_or("1000")
//   .parse::<usize>()
//   .unwrap();

//   let batch_size = matches
//   .value_of("batch_size")
//   .unwrap_or("10")
//   .parse::<usize>()
//   .unwrap();

//   let (vertex_output_sender, vertex_output_receiver) = channel::<Vertex>(DEFAULT_CHANNEL_CAPACITY);

//   let (vertex_to_broadcast_sender, vertex_to_broadcast_receiver) = channel::<Vertex>(DEFAULT_CHANNEL_CAPACITY);
//   let (vertex_to_consensus_sender, vertex_to_consensus_receiver) = channel::<Vertex>(DEFAULT_CHANNEL_CAPACITY);
//   let (block_sender, block_receiver) = channel::<Block>(DEFAULT_CHANNEL_CAPACITY);

//   VertexCoordinator::spawn(
//       node_id,
//       Committee::default(),
//       vertex_to_consensus_sender,
//       vertex_to_broadcast_receiver
//   );

//   TransactionCoordinator::spawn(
//       node_id,
//       Committee::default(),
//       block_sender,
//       batch_size,
//   );

//   Consensus::spawn(
//       node_id,
//       Committee::default(),
//       vertex_to_consensus_receiver,
//       vertex_to_broadcast_sender,
//       vertex_output_sender,
//       block_receiver
//   );

//   wait_and_print_vertexs(vertex_output_receiver).await;
//   unreachable!();
// }