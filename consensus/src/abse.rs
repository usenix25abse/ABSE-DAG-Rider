use std::collections::VecDeque;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct ABSE {
    r: u64,  // record the number of rounds, initially 0
    f: u64,  // record the maxium number of faulties 
    s: Vec<u64>,  // the score of p_i
    ref_s: Vec<u64>,  // the reference score of p_i
    scores_i: VecDeque<Vec<u64>>,  // A queue of s, initially all 0
    info: Vec<u64>,  // A structure used to record voting information
    baseline: f64,  // the baseline score of leader election
    size: usize,  // The size for checking the queue length (confirm)
}

impl ABSE {
    pub fn new(size: usize, f:u64) -> ABSE {
        ABSE {
            r: 0,
            f,
            s: Vec::new(),
            ref_s: Vec::new(),
            scores_i: VecDeque::new(),
            info: Vec::new(),  // TODO: Initialize with actual voting information
            baseline: 0.0,
            size,
        }
    }

    pub fn set_info(&mut self, info: Vec<u64>) {
      if info.is_empty(){
        self.info = Vec::new();
      }else{
        self.info = info;
      }
    }

    pub fn merge_info(&mut self, info: Vec<u64>) {
      let mut minfo = info.clone();
      if self.info.len() > minfo.len() {
        minfo.resize(self.info.len(), 0);
      } else if self.info.len() < minfo.len() {
          self.info.resize(minfo.len(), 0);
      }

      let new_s = self.info.iter().zip(minfo.iter()).map(|(a, b)| a + b).collect::<Vec<u64>>();
      self.set_info(new_s);
    }

    pub fn generate(&mut self) -> Result<Vec<u64>, Box<dyn Error>> {
        //let rear_data = self.scores_i.back().unwrap();
        let mut rear_data = self.scores_i.back().cloned().unwrap_or_else(|| vec![0]);

        if self.info.len() > rear_data.len() {
          rear_data.resize(self.info.len(), 0);
        } else if self.info.len() < rear_data.len() {
            self.info.resize(rear_data.len(), 0);
        }

        let new_s = self.info.iter().zip(rear_data.iter()).map(|(a, b)| a + b).collect::<Vec<u64>>();
        Ok(new_s)
    }

    // pub fn update(&mut self, s: Vec<u64>, f: f64) {
    //     if self.scores_i.len() >= self.size {
    //         self.ref_s = self.scores_i.pop_front().unwrap();
    //     }
    //     self.scores_i.push_back(s);
    //     // A new baseline is obtained based on r. The computation rules can be specialised for different scenarios.
    //     self.baseline = (self.r as f64 * (2.0 * f + 1.0)) / (3.0 * f + 1.0);
    // }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
      let s = self.generate()?;
      if self.scores_i.len() >= self.size {
          self.ref_s = self.scores_i.pop_front().unwrap();
      }
      self.scores_i.push_back(s);
      // A new baseline is obtained based on r. The computation rules can be specialized for different scenarios.
      //self.baseline = (self.r as f64 * (2.0 * f + 1.0)) / (3.0 * f + 1.0);
      self.baseline = ((self.r as f64 - self.size as f64 - 1.0).max(0.0)) * (2 * self.f + 1) as f64 / (3 * self.f + 1) as f64 / 6.0;
      // On a 4 rounds (a wave) basis.
      Ok(())
    }

    pub fn judge(&self, j: usize) -> bool {
        if self.ref_s.is_empty() || self.ref_s.len() < j+1 || self.ref_s[j] as f64 >= self.baseline.floor() {
            true
        } else {
            false
        }
    }
    pub fn update_round(&mut self, r: u64) {
      self.r = r;
  }
    pub fn get_r(&self) -> u64{
      self.r
    }
}

#[cfg(test)]
mod tests {
    use super::ABSE;

    #[test]
    fn test_abse_new() {
        let abse = ABSE::new(5,2);
        assert_eq!(abse.size, 5);
        assert_eq!(abse.scores_i.len(), 0);
    }

    #[test]
    fn test_abse_generate() {
        let mut abse = ABSE::new(2,2);
        // abse.scores_i.push_back(vec![1, 2, 3]);
        // abse.scores_i.push_back(vec![2, 3, 4]);
        abse.set_info(vec![1, 2, 3]);
        let result = abse.generate();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    // #[test]
    // fn test_abse_update() {
    //     let mut abse = ABSE::new(2);
    //     abse.scores_i.push_back(vec![1, 2, 3]);
    //     abse.scores_i.push_back(vec![4, 5, 6]);
    //     abse.r = 3;
    //     abse.update(vec![7, 8, 9], 2.0);
    //     assert_eq!(abse.scores_i.len(), 2);
    //     assert_eq!(abse.scores_i[0], vec![4, 5, 6]);
    //     assert_eq!(abse.scores_i[1], vec![7, 8, 9]);
    //     assert_eq!(abse.ref_s, vec![1, 2, 3]);
    //     //assert_eq!(abse.baseline, 2.25);
    //     abse.r = 4;
    //     abse.update(vec![1, 1, 1], 2.0);
    //     assert_eq!(abse.ref_s, vec![4, 5, 6]);
    // }

    #[test]
    fn test_abse_update() {
        let mut abse = ABSE::new(2,2);
        abse.update_round(4);
        //abse.set_info(vec![1, 2, 3]);
        abse.set_info(Vec::new());
        abse.update();
        assert_eq!(abse.scores_i.len(), 1);
        //assert_eq!(abse.scores_i[0], vec![1, 2, 3]);
        abse.set_info(Vec::new());
        abse.update();
        //assert_eq!(abse.scores_i[1], vec![1, 2, 3]);
        //assert_eq!(abse.scores_i[1], vec![7, 8, 9]);
        //assert_eq!(abse.ref_s, vec![1, 2, 3]);
        assert_eq!(abse.baseline, 2.25);
    }

    #[test]
    fn test_abse_judge() {
        let mut abse = ABSE::new(2,2);
        //abse.ref_s = vec![1.0, 2.0, 3.0];
        abse.update_round(1);
        //abse.set_info(Vec::new());
        abse.update();
        assert_eq!(abse.judge(0), true);
        assert_eq!(abse.judge(1), true);
        assert_eq!(abse.judge(2), true);
    }
}