mod utils;
use bitcoincore_rpc::RpcApi;
use tracker::bitcoin_client::BitcoinMintExt;
use tracker::index::BagId;
use tracker::storage::sqlite::SqliteIndexStorage;
use tracker::storage::IndexStorage;
use tracker::Index;
use utils::*;

const GENERATED_BLOCKS: u64 = 120;

macro_rules! wait2 {
    ($cond:expr) => {
        assert!(wait_until(2, || $cond));
    };
}

#[test]
fn test_reorg_longest_chain() {
    let storage = SqliteIndexStorage::in_memory();

    let _tempdir = TempDir::new("/tmp/test_reorg_longest_chain/".to_string());
    let dir1 = "/tmp/test_reorg_longest_chain/node1/";
    let dir2 = "/tmp/test_reorg_longest_chain/node2/";

    const NODE1_PORT: u16 = 18444 + 3;
    const NODE2_PORT: u16 = 18444 + 4;
    let _node1_addr = format!("localhost:{}", NODE1_PORT);
    let node2_addr = format!("localhost:{}", NODE2_PORT);

    let (_dir1, _child1, client1, address1) = init_client(dir1, GENERATED_BLOCKS, 3);
    let (_dir2, _child2, client2, address2) = init_client(dir2, 0, 4);

    const BAG1_12: BagId = [1; 32]; // bag #1 on both chains
    const BAG2_1: BagId = [2; 32]; // bag #2 on chain #1
    const BAG2_2: BagId = [3; 32]; // bag #2 on chain #2
    const BAG3_2: BagId = [4; 32]; // bag #3 on chain #2

    // Connect node1 to node2
    assert_eq!(client1.get_network_info().unwrap().connections, 0);
    add_node_client(&client1, &node2_addr);
    wait2!(client1.get_network_info().unwrap().connections == 1);
    wait2!(client2.get_blockchain_info().unwrap().blocks == GENERATED_BLOCKS);
    client2
        .generate_to_address(GENERATED_BLOCKS, &address2)
        .unwrap();
    wait2!(client1.get_blockchain_info().unwrap().blocks == GENERATED_BLOCKS * 2);

    // Both nodes have mint tx
    let tx_id = client1.send_mint_transaction(10, &BAG1_12).unwrap();
    let both_block = generate_block(&client1, &address1, &tx_id);
    // Wait before node2 receive block
    assert!(wait_until(2, || client2
        .get_blockchain_info()
        .unwrap()
        .best_block_hash
        == both_block));

    // Disconnect nodes
    disconnect_node_client(&client1, &node2_addr);
    assert!(wait_until(2, || client1
        .get_network_info()
        .unwrap()
        .connections
        == 0));

    // Mine block with Bag2_1 on node 1
    let last_block_chain_1 = {
        let tx_id = client1.send_mint_transaction(10, &BAG2_1).unwrap();
        generate_block(&client1, &address1, &tx_id)
    };

    let (bag2_2block, bag3_2block) = {
        // Mine block with Bag2_2 on node 2
        let tx_id = client2.send_mint_transaction(10, &BAG2_2).unwrap();
        let bag1block = generate_block(&client2, &address2, &tx_id);

        // Mine block with Bag3_2 on node 2
        let tx_id = client2.send_mint_transaction(10, &BAG3_2).unwrap();
        let bag2block = generate_block(&client2, &address2, &tx_id);

        (bag1block, bag2block)
    };

    let mut index = Index::new(client1, storage, Some(119));
    index.check_last_btc_blocks();

    // Check that node1 contains only 2 bags on chain #1
    {
        assert_eq!(index.checked_btc_height(), GENERATED_BLOCKS * 2 + 2);

        let store = index.get_storage();
        assert_eq!(store.get_blocks_count().unwrap(), 2);

        let txs = store
            .get_records_by_block_hash(&last_block_chain_1)
            .unwrap();
        assert_eq!(txs.last().unwrap().data.bag_id, BAG2_1);
    }

    // Reconnect node1 with node2
    let client1 = index.btc_client();
    add_node_client(client1, &node2_addr);
    assert_eq!(client1.get_network_info().unwrap().connections, 1);
    wait2!(client1.get_blockchain_info().unwrap().blocks == GENERATED_BLOCKS * 2 + 3);

    // Tracker must find reorg there
    index.check_last_btc_blocks();

    // Check that reorg happened and chain #2 is main now
    {
        assert_eq!(index.checked_btc_height(), GENERATED_BLOCKS * 2 + 3);

        let store = index.get_storage();
        assert_eq!(store.get_blocks_count().unwrap(), 3);

        let txs = store.get_records_by_block_hash(&bag3_2block).unwrap();
        assert_eq!(txs.last().unwrap().data.bag_id, BAG3_2);

        let txs = store.get_records_by_block_hash(&bag2_2block).unwrap();
        assert_eq!(txs.last().unwrap().data.bag_id, BAG2_2);
    }
}