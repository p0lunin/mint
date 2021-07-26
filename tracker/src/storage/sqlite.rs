use crate::storage::{IndexStorage, Record, RecordData};
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use bitcoin::Txid;
use rusqlite::blob::ZeroBlob;
use rusqlite::{Connection, DatabaseName};
use std::convert::TryFrom;
use std::path::Path;

#[derive(Debug)]
pub struct SqliteIndexStorage {
    connection: Connection,
}

impl SqliteIndexStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self::with_connection(Connection::open(path).unwrap())
    }
    pub fn in_memory() -> Self {
        Self::with_connection(Connection::open_in_memory().unwrap())
    }
    pub fn with_connection(connection: Connection) -> Self {
        let this = SqliteIndexStorage { connection };
        this.init_tables();
        this
    }
}

impl SqliteIndexStorage {
    fn init_tables(&self) {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS records (
             block BLOB,
             txid BLOB,
             out_pos INTEGER,
             bag_id BLOB,
             amount INTEGER
         )",
                [],
            )
            .unwrap();
    }

    fn write_blob(&self, column: &str, row_id: i64, data: &[u8]) -> Result<(), rusqlite::Error> {
        let mut blob =
            self.connection
                .blob_open(DatabaseName::Main, "records", column, row_id, false)?;
        blob.write_at(data, 0)?;
        Ok(())
    }
}

impl IndexStorage for SqliteIndexStorage {
    type Err = rusqlite::Error;

    fn store_record(&self, record: Record) -> Result<(), Self::Err> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                ZeroBlob(32),
                ZeroBlob(32),
                record.bitcoin_output_position,
                ZeroBlob(32),
                record.data.amount
            ],
        )?;
        let rowid = self.connection.last_insert_rowid();
        self.write_blob("block", rowid, record.bitcoin_block.as_ref())?;
        self.write_blob("txid", rowid, record.bitcoin_tx_id.as_ref())?;
        self.write_blob("bag_id", rowid, &record.data.bag_id)?;
        Ok(())
    }

    fn get_blocks_count(&self) -> Result<u64, Self::Err> {
        self.connection
            .query_row("SELECT COUNT(*) FROM records;", [], |row| row.get(0))
    }

    fn remove_with_block_hash(&self, _hash: &BlockHash) -> Result<(), Self::Err> {
        unimplemented!()
    }

    fn get_blocks_by_hash(&self, hash: &BlockHash) -> Result<Vec<Record>, Self::Err> {
        let mut stmt = self
            .connection
            .prepare("SELECT block, txid, out_pos, bag_id, amount FROM records WHERE block = ?1")?;

        let res = stmt.query_map([hash.as_ref()], |row| {
            Ok(Record {
                bitcoin_block: {
                    let vec: Vec<u8> = row.get(0)?;
                    BlockHash::from_slice(&vec).expect("TODO: handle the error")
                },
                bitcoin_tx_id: {
                    let vec: Vec<u8> = row.get(1)?;
                    Txid::from_slice(&vec).expect("TODO: handle the error")
                },
                bitcoin_output_position: row.get(2)?,
                data: RecordData {
                    bag_id: {
                        let vec: Vec<u8> = row.get(3)?;
                        TryFrom::try_from(vec.as_slice()).expect("TODO: handle the error")
                    },
                    amount: row.get(4)?,
                },
            })
        });

        res.and_then(|cursor| cursor.collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::RecordData;
    use bitcoin::Txid;

    #[test]
    fn sqlite_storage_tests() {
        let store = SqliteIndexStorage::in_memory();

        let record = dummy_record([1; 32], [2; 32], 4, [3; 32], 5);

        store.store_record(record.clone()).unwrap();

        assert_eq!(store.get_blocks_count().unwrap(), 1);

        let records = store.get_blocks_by_hash(&record.bitcoin_block).unwrap();
        assert_eq!(records, vec![record.clone()]);
    }

    fn dummy_record(
        block: [u8; 32],
        txid: [u8; 32],
        out_pos: u64,
        bag_id: [u8; 32],
        amount: u64,
    ) -> Record {
        Record {
            bitcoin_block: BlockHash::hash(&block),
            bitcoin_tx_id: Txid::hash(&txid),
            bitcoin_output_position: out_pos,
            data: RecordData { bag_id, amount },
        }
    }
}
