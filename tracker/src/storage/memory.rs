use crate::record::BagEntry;
use crate::storage::IndexStorage;
use bitcoin::BlockHash;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;

#[derive(Debug)]
pub struct MemoryIndexStorage(RefCell<HashMap<BlockHash, Vec<BagEntry>>>);

impl MemoryIndexStorage {
    pub fn new() -> Self {
        MemoryIndexStorage(RefCell::new(HashMap::new()))
    }
}

impl IndexStorage for MemoryIndexStorage {
    type Err = Infallible;

    fn store_record(&self, record: BagEntry) -> Result<(), Self::Err> {
        let mut this = self.0.borrow_mut();
        let vec = this.entry(record.btc_block).or_default();
        vec.push(record);
        Ok(())
    }

    fn get_blocks_count(&self) -> Result<u64, Self::Err> {
        Ok(self.0.borrow().len() as u64)
    }

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err> {
        self.0.borrow_mut().remove(hash);
        Ok(())
    }

    fn get_records_by_block_hash(&self, hash: &BlockHash) -> Result<Vec<BagEntry>, Self::Err> {
        let this = self.0.borrow();
        let records = this.get(hash).map(Clone::clone).unwrap();
        Ok(records)
    }

    fn remove_records_with_bag(&self, bag: &[u8; 32]) -> Result<(), Self::Err> {
        // MemoryIndexStorage is used only to debug so no need to worry about performance
        let mut this = self.0.borrow_mut();

        let mut keys_with_empty = vec![];

        for (key, records) in this.iter_mut() {
            let mut i = 0;
            while i != records.len() {
                if records[i].data.bag_id == *bag {
                    records.remove(i);
                } else {
                    i += 1;
                }
            }
            if records.len() == 0 {
                keys_with_empty.push(key.clone());
            }
        }

        keys_with_empty.into_iter().for_each(|key| {
            this.remove(&key);
        });

        Ok(())
    }
}
