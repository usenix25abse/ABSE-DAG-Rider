use std::collections::{HashMap};
use std::net::SocketAddr;
use ed25519_dalek::Keypair;
use serde::{Serialize, Deserialize};


pub type Id = u32;
pub type NodePublicKey = [u8; 32];

#[derive(Clone, Serialize, Deserialize)]
pub struct Validator {
    pub address: SocketAddr,
    pub tx_address: SocketAddr,
    pub block_address: SocketAddr,
    pub public_key: NodePublicKey,
}

impl Validator {
    pub fn new(keypair: &str, port: u16, tx_port: u16, block_port: u16) -> Self {
        let keypair = Validator::create_keypair(String::from(keypair));
        let public_key = Validator::create_node_public_key_from(&keypair);
        Self {
            address: SocketAddr::new("0.0.0.0".parse().unwrap(), port),
            tx_address: SocketAddr::new("0.0.0.0".parse().unwrap(), tx_port),
            block_address: SocketAddr::new("0.0.0.0".parse().unwrap(), block_port),
            public_key,
        }
    }

    fn create_keypair(kps: String) -> Keypair {
        let bytes = hex::decode(kps).unwrap();
        return Keypair::from_bytes(&bytes).unwrap();
    }

    fn create_node_public_key_from(keypair: &Keypair) -> NodePublicKey {
        let encoded = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Committee {
    pub validators: HashMap<Id, Validator>,
}

impl Committee {
    pub fn default() -> Self {
        let mut validators = HashMap::new();
        validators.insert(1, Validator::new(
            "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416",
            1234, 1244, 1254));
        validators.insert(2, Validator::new(
            "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd",
            1235, 1245, 1255));
        validators.insert(3, Validator::new(
            "6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b",
            1236, 1246, 1256));
        validators.insert(4, Validator::new(
            "3ae38eec96146c241f6cadf01995af14f027b23b8fecbc77dbc2e3ed5fec6fc3fb4fe5534f7affc9a8f1d99e290fdb91cc26777edd6fae480cad9f735d1b3680",
            1237, 1247, 1257));

        Self {
            validators
        }
    }

    pub fn generate(node_count: u32) -> Self {
      let mut validators = HashMap::new();

      for id in 1..=node_count {
          let keypair = Keypair::generate(&mut rand::thread_rng());
          let kp_bytes = keypair.to_bytes();
          let kp_hex = hex::encode(kp_bytes);

          // Assign port numbers for each type of address.
          let port: u16 = 8123 + ((id as u16 - 1) * 3) as u16;
          let tx_port: u16 = port + 1;
          let block_port: u16 = port + 2;

          validators.insert(id, Validator::new(&kp_hex, port, tx_port, block_port));
      }

      Self { validators }
    }

    pub fn size(&self) -> usize {
        self.validators.len()
    }

    pub fn quorum_threshold(&self) -> usize {
        (self.size() * 2 / 3) + 1
    }

    pub fn get_node_address(&self, id: Id) -> Option<SocketAddr> {
        match self.validators.get(&id) {
            Some(v) => Some(v.address),
            None => None
        }
    }

    pub fn get_node_addresses(&self) -> Vec<SocketAddr> {
        self.validators.iter().map(|v| v.1.address).collect()
    }

    pub fn get_tx_receiver_address(&self, id: Id) -> Option<SocketAddr> {
        self.validators.get(&id).map(|v| v.tx_address)
    }

    pub fn get_tx_receiver_addresses(&self) -> Vec<SocketAddr> {
        self.validators.iter().map(|v| v.1.tx_address).collect()
    }

    pub fn get_block_receiver_address(&self, id: Id) -> Option<SocketAddr> {
        self.validators.get(&id).map(|v| v.block_address)
    }

    pub fn get_block_receiver_addresses(&self) -> Vec<SocketAddr> {
        self.validators.iter().map(|v| v.1.block_address).collect()
    }

    pub fn get_node_addresses_but_me(&self, id: Id) -> Vec<SocketAddr> {
        self.validators.iter().filter(|v| *v.0 != id).map(|v| v.1.address).collect()
    }

    pub fn get_nodes_keys(&self) -> Vec<NodePublicKey> {
        self.validators.iter().map(|v| v.1.public_key.clone()).collect()
    }

    pub fn get_node_key(&self, id: Id) -> Option<NodePublicKey> {
        self.validators.get(&id).map(|v| v.public_key)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_committee_generate() {
        let committee = Committee::generate(10);

        // Check if the committee has the correct number of validators.
        assert_eq!(committee.validators.len(), 10);

        // Check if the port numbers are assigned correctly.
        for (id, validator) in committee.validators {
            assert_eq!(validator.address.port(), 8123 + ((id as u16 - 1) * 3));
            assert_eq!(validator.tx_address.port(), 8124 + ((id as u16 - 1) * 3));
            assert_eq!(validator.block_address.port(), 8125 + ((id as u16 - 1) * 3));
        }
    }
}