use std::collections::HashSet;
use std::collections::HashMap;
use log::{debug, info};
use tokio::sync::mpsc::{Receiver, Sender};

use model::{Round, Wave};
use model::block::Block;
use model::committee::{Committee, Id, NodePublicKey};
use model::vertex::{Vertex, VertexHash};

use crate::state::State;
use crate::abse::ABSE;

mod dag;
mod state;
mod abse;

const MAX_WAVE: Wave = 4;

pub struct Consensus {
    node_id: Id,
    committee: Committee,
    decided_wave: Wave,
    state: State,
    delivered_vertices: HashSet<VertexHash>,
    buffer: Vec<Vertex>,
    blocks_to_propose: Vec<Block>,
    blocks_receiver: Receiver<Block>,
    vertex_receiver: Receiver<Vertex>,
    vertex_output_sender: Sender<Vertex>,
    vertex_to_broadcast_sender: Sender<Vertex>,
    abse_struct:ABSE,
    score_array: Vec<u64>,
    id_to_index: HashMap<NodePublicKey, usize>,
    ftype: usize,
    advstra: Vec<Vertex>,
}

impl Consensus {
    pub fn spawn(
        node_id: Id,
        committee: Committee,
        vertex_receiver: Receiver<Vertex>,
        vertex_to_broadcast_sender: Sender<Vertex>,
        vertex_output_sender: Sender<Vertex>,
        blocks_receiver: Receiver<Block>,
        ftype: usize,
    ) {
        tokio::spawn(async move {
            let state = State::new(Vertex::genesis(committee.get_nodes_keys()));
            let csize = committee.size().clone();
            let score_array = vec![0; csize];
            let faulties = csize - committee.quorum_threshold().clone();
            let mut id_to_index = HashMap::new();
            for (_, validator) in committee.validators.iter() {
              let public_key = &validator.public_key;
              id_to_index.insert(public_key.clone(), id_to_index.len());
            }
            Self {
                node_id,
                committee,
                vertex_receiver,
                vertex_output_sender,
                vertex_to_broadcast_sender,
                decided_wave: 0,
                state,
                delivered_vertices: HashSet::new(),
                buffer: vec![],
                blocks_to_propose: vec![],
                blocks_receiver,
                abse_struct: ABSE::new(3, faulties as u64),
                score_array,
                id_to_index,
                ftype,
                advstra: vec![],
            }.run().await;
        });
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(vertex) = self.vertex_receiver.recv() => {
                    debug!("Vertex received in consensus of 'node {}': {}", self.node_id, vertex);
                    self.buffer.push(vertex);

                    // Go through buffer and add vertex in the dag which meets the requirements
                    // and remove from the buffer those added
                    self.buffer.retain(|v| {
                        if v.round() <= self.state.current_round && self.state.dag.contains_vertices(v.parents()) {
                        // if v.round() <= self.state.current_round {
                            self.state.dag.insert_vertex(v.clone());
                            false
                        } else {
                            true
                        }
                    })
                },
                Some(block) = self.blocks_receiver.recv() => {
                    self.blocks_to_propose.push(block)
                }
            }

            debug!("Consensus goes to the next iteration");
            //debug!("block to propose:{}",self.blocks_to_propose.is_empty());

            if !self.blocks_to_propose.is_empty() && self.state.dag.is_quorum_reached_for_round(&(self.state.current_round)) {
                info!("DAG has reached the quorum for the round {:?}", self.state.current_round);
                if Self::is_last_round_in_wave(self.state.current_round) {
                    info!("Finished the last round {:?} in the wave. Start to order vertices", self.state.current_round);
                    let ordered_vertices = self.get_ordered_vertices(self.state.current_round / MAX_WAVE);

                    info!("Got {} vertices to order", ordered_vertices.len());
                    for vertex in ordered_vertices {
                        self.vertex_output_sender
                            .send(vertex.clone())
                            .await
                            .expect("Failed to output vertex");
                    }
                }
                // when quorum for the round reached, then go to the next round

                self.state.current_round += 1;
                let current_round = self.state.current_round.clone();
                debug!("DAG goes to the next round {:?},
                the DAG graph shown below contains both strong and weak edges 
                 \n{}", self.state.current_round, self.state.dag);
                
                if self.abse_struct.get_r() < current_round {
                  let s_array = self.get_array().to_vec();
                  debug!("Success! Current array is: {:?}", s_array);
                  self.abse_struct.set_info(s_array);
                  self.reset_array();
                  self.abse_struct.update_round(current_round);
                  self.abse_struct.update();
                  self.abse_struct.set_info(Vec::new());
                  debug!("{:?}: ABSE Struct", self.abse_struct);
                }
                
                if self.ftype == 2 {
                  if !self.advstra.is_empty(){
                    let advstracp = self.advstra.pop().unwrap();
                    let new_vertex = self.create_new_vertex_adv(self.state.current_round, advstracp.hash().clone(), advstracp.round().clone()).await.unwrap();
                    self.advstra.push(advstracp);
                    self.advstra.push(new_vertex);
                  }else{
                    let new_vertex = self.create_new_vertex(self.state.current_round).await.unwrap();
                    self.advstra.push(new_vertex);
                  }
                  
                  if Self::is_last_round_in_wave(self.state.current_round) {
                    while !self.advstra.is_empty(){
                      let vert = self.advstra.pop().unwrap();
                      info!("Broadcast the new vertex {}", vert);
                      self.vertex_to_broadcast_sender.send(vert).await.unwrap();
                    }
                  }
                }else{
                  let new_vertex = self.create_new_vertex(self.state.current_round).await.unwrap();

                  info!("Broadcast the new vertex {}", new_vertex);
                  self.vertex_to_broadcast_sender.send(new_vertex).await.unwrap();
                }
                // let new_vertex = self.create_new_vertex(self.state.current_round).await.unwrap();

                // info!("Broadcast the new vertex {}", new_vertex);
                // self.vertex_to_broadcast_sender.send(new_vertex).await.unwrap();
                //debug!("Broadcast the new vertex successfully!");
            }
        }
    }

    async fn create_new_vertex(&mut self, round: Round) -> Option<Vertex> {
        let block = self.blocks_to_propose.pop().unwrap();
        info!("Start to create a new vertex with the block and {} transactions", block.transactions.len());
        let parents = self.state.dag.get_vertices(&(round - 1));
        let mut vertex = Vertex::new(
            self.committee.get_node_key(self.node_id).unwrap(),
            round,
            block,
            parents,
        );

        if round > 2 {
            self.set_weak_edges(&mut vertex, round);
        }

        return Some(vertex);
    }

    async fn create_new_vertex_adv(&mut self, round: Round, vh: VertexHash, lr: Round) -> Option<Vertex> {
      let block = self.blocks_to_propose.pop().unwrap();
      info!("Start to create a new vertex with the block and {} transactions", block.transactions.len());
      let mut parents = self.state.dag.get_vertices(&(round - 1));
      parents.insert(vh,lr);
      let mut vertex = Vertex::new(
          self.committee.get_node_key(self.node_id).unwrap(),
          round,
          block,
          parents,
      );

      if round > 2 {
          self.set_weak_edges(&mut vertex, round);
      }

      return Some(vertex);
    }

    fn set_weak_edges(&self, vertex: &mut Vertex, round: Round) {
        for r in (1..round - 2).rev() {
            if let Some(vertices) = self.state.dag.graph.get(&r) {
                for (_, v) in vertices {
                    if !self.state.dag.is_linked(&vertex, v) {
                        vertex.add_parent(v.hash(), r)
                    }
                }
            }
        }
    }

    fn get_ordered_vertices(&mut self, wave: Wave) -> Vec<Vertex> {
        if let Some(leader) = self.get_wave_vertex_leader(wave) {
            let wleader = leader.clone();
            debug!("Selected a vertex leader: {}", leader);
            // we need to make sure that if one correct process commits the wave
            // vertex leader 𝑣, then all the other correct processes will commit 𝑣
            // later. To this end, we use standard quorum intersection. Process 𝑝𝑖
            // commits the wave 𝑤 vertex leader 𝑣 if:
            let round = self.get_round_for_wave(wave, MAX_WAVE);
            if self.state.dag.is_linked_with_others_in_round(leader, round) {
                let linked_public_keys = self.state.dag.get_valid_vertices_voters(leader, round);
                debug!("The leader is strongly linked to others in the round {}", round);
                let mut leaders_to_commit = self.get_leaders_to_commit(wave - 1, leader);
                self.decided_wave = wave;
                debug!("Set decided wave to {}", wave);
                for pubkey in linked_public_keys {
                  self.set_voter_id(pubkey);
                }
                self.set_voter_id(wleader.owner());
                // go through the un-committed leaders starting from the oldest one
                return self.order_vertices(&mut leaders_to_commit);
            }
        }
        return vec![];
    }

    fn get_leaders_to_commit(&self, from_wave: Wave, current_leader: &Vertex) -> Vec<Vertex> {
        let mut to_commit = vec![current_leader.clone()];
        let mut current_leader = current_leader;

        if from_wave > 0 {
            // Go for each wave up until decided_wave and find which leaders we need to commit
            for wave in (from_wave..self.decided_wave + 1).rev()
            {
                // Get the vertex proposed in the previous wave.
                debug!("Get the vertex proposed in the previous wave.");
                if let Some(prev_leader) = self.get_wave_vertex_leader(wave) {
                    // if no strong link between leaders then skip for this wave
                    // and maybe next time there will be a strong link
                    if self.state.dag.is_strongly_linked(current_leader, prev_leader) {
                        to_commit.push(prev_leader.clone());
                        current_leader = prev_leader;
                    }
                }
            }
        }
        to_commit
    }

    fn order_vertices(&mut self, leaders: &mut Vec<Vertex>) -> Vec<Vertex> {
        let mut ordered_vertices = Vec::new();

        // go from the oldest leader to the newest by taking items from the tail
        while let Some(leader) = leaders.pop() {
            debug!("Start ordering vertices from the leader: {:?}", leader);

            for (round, vertices) in &self.state.dag.graph {
                if *round > 0 {
                    for vertex in vertices.values() {
                        let vertex_hash = vertex.hash();
                        if !self.delivered_vertices.contains(&vertex_hash) && self.state.dag.is_linked(vertex, &leader) {
                            ordered_vertices.push(vertex.clone());
                            self.delivered_vertices.insert(vertex_hash);
                        }
                    }
                }
            }
        }

        ordered_vertices
    }

    fn get_wave_vertex_leader(&self, wave: Wave) -> Option<&Vertex> {
        let first_round_of_wave = self.get_round_for_wave(wave, 1);
        // let coin = first_round_of_wave;
         let coin = wave;

        // Elect the leader.
        let mut keys: Vec<_> = self.committee.get_nodes_keys();
        keys.sort();
        let leader = keys[coin as usize % self.committee.size()];
        let abse_s = self.abse_struct.clone();
        if let Some(index) = self.get_index(leader.clone()){
          if abse_s.judge(index) {
            if self.choose_leader(wave.clone()){
              debug!("{}-{:?}: can be the leader of wave {}", index, leader, wave);
              self.state.dag.graph.get(&first_round_of_wave).map(|x| x.get(&leader)).flatten()
            }else{
              None
            }
          }else {
            debug!("{}-{:?}: can not be the leader of wave {}, skip.", index, leader, wave);
            None
          }
        }else{
          None
        }
        //self.state.dag.graph.get(&first_round_of_wave).map(|x| x.get(&leader)).flatten()
    }

    fn choose_leader(&self, wave: Wave) -> bool {
    // //  Since the existing DAG protocols only emulate the 
    // // building block of global perfect coin, we also choose to emulate 'choose_leader', 
    // // but of course, we can also choose to use the following annotated form of broadcasting 
    // // to realize it 
    //
    //   self.leader_choose_broadcaster.send(LeaderMessage::::new(
    //     self.committee.get_node_key(self.node_id).unwrap(),
    //     wave,
    //   ));
    //   let wave_map = self.leadermessages.entry(wave).or_default();
    //   if wave_map.len() >= self.committee.quorum_threshold() {
    //     true
    //   }else{
    //     false
    //   }
    //   // An additional thread would be needed to always perform 
    //   // the function similar to the code below, 
    //   // specifically by defining a leader_choose_coordinator like structure 
    //   // and then spawn_run in main:
    //
    //   tokio::select! {
    //     Some(lm) = self.leader_choose_receiver.recv() => {
    //         let voter_id = lm.public_key;
    //         // The type of leadermessages is similar to 
    //         // pub leadermessages: HashMap<Wave, Vec<PublicKey>>,
    //         let wave_map = self.leadermessages.entry(wave).or_default();
    //         // TODO: check if voter_id is already in voters
    //         wave_map.push(voter_id);
    //     },
    // }
      true
    }

    fn get_round_for_wave(&self, wave: Wave, round: Round) -> Round {
        (MAX_WAVE * (wave - 1) + round) as Round
    }

    fn is_last_round_in_wave(round: Round) -> bool {
        round % MAX_WAVE == 0
    }

    fn get_index(&self, voter_id: NodePublicKey) -> Option<usize> {
      self.id_to_index.get(&voter_id).cloned()
    }

    fn reset_array(&mut self) {
      for item in &mut self.score_array {
          *item = 0;
      }
    }
  
    fn get_array(&self) -> &[u64] {
      &self.score_array
    }

    fn set_voter_id(&mut self, voter_id: NodePublicKey) {
      //debug!("Set for: {:?}!",voter_id);
      if let Some(index) = self.get_index(voter_id) {
          self.score_array[index] += 1;
          //debug!("Success! Current array is: {:?}", self.score_array);
      }
    }
}
