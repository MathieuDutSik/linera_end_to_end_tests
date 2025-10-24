use anyhow::Result;
use linera_base::{
    time::Instant,
};
use linera_views::{
    batch::Batch,
    dynamo_db::DynamoDbDatabase,
    rocks_db::RocksDbDatabase,
    scylla_db::ScyllaDbDatabase,
    store::{
        KeyValueStore,
        ReadableKeyValueStore,
        TestKeyValueDatabase,
        WritableKeyValueStore,
    },
};
use rand::SeedableRng;

use rand::Rng;
use std::env;

fn get_key(rng: &mut impl Rng, len: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..len {
        let value = rng.gen::<u8>();
        v.push(value);
    }
    v
}



async fn test_storage_multi_read_kernel<S>(num_key: usize, key_size: usize, value_size: usize) -> Result<()>
where
    S: TestKeyValueDatabase,
    S::Store: Clone + KeyValueStore,
{
    let store = S::new_test_store().await?;
    let name = S::get_name();
    let mut batch = Batch::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(134 as u64);
    let mut keys1 = Vec::new();
    let mut keys2 = Vec::new();
    let mut keys3 = Vec::new();
    let mut read_values = Vec::new();
    for _ in 0..num_key {
        let key = get_key(&mut rng, key_size);
        let value = get_key(&mut rng, value_size);
        keys1.push(key.clone());
        keys2.push(key.clone());
        keys3.push(key.clone());
        read_values.push(Some(value.clone()));
        batch.put_key_value_bytes(key, value);
    }
    store.write_batch(batch).await?;
    //
    let time = Instant::now();
    let values: Vec<Option<Vec<u8>>> = store.read_multi_values_bytes(keys1).await?;
    println!("Runtime {name} for multi_read: {}ms", time.elapsed().as_micros() as f64);
    assert_eq!(values, read_values);
    //
    let time = Instant::now();
    let mut values: Vec<Option<Vec<u8>>> = Vec::new();
    for key in keys2 {
        values.push(store.read_value_bytes(&key).await?);
    }
    println!("Runtime {name} for loop read: {}ms", time.elapsed().as_micros() as f64);
    assert_eq!(values, read_values);
    //
    let time = Instant::now();
    let mut futures = Vec::new();
    for key in keys3 {
        let store = store.clone();
        futures.push(async move { store.read_value_bytes(&key).await });
    }
    let values: Vec<Option<Vec<u8>>> = futures::future::try_join_all(futures).await?;
    println!("Runtime {name} for futures read: {}ms", time.elapsed().as_micros() as f64);
    assert_eq!(values, read_values);
    //
    Ok(())
}

async fn test_various_storage(num_key: usize, key_size: usize, value_size: usize) -> Result<()> {
    test_storage_multi_read_kernel::<DynamoDbDatabase>(num_key, key_size, value_size).await?;
    test_storage_multi_read_kernel::<ScyllaDbDatabase>(num_key, key_size, value_size).await?;
    test_storage_multi_read_kernel::<RocksDbDatabase>(num_key, key_size, value_size).await?;
    Ok(())
}




#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        eprintln!("Error: argument specified");
        eprintln!("Usage: {} [num_key] [key_size] [value_size]", args[0]);
        eprintln!("e.g. num_key = 100, key_size = 10, value_size = 10000");
        std::process::exit(1);
    }

    let num_key = &args[1];
    let key_size = &args[2];
    let value_size = &args[3];

    let num_key = num_key.parse::<usize>()?;
    let key_size = key_size.parse::<usize>()?;
    let value_size = value_size.parse::<usize>()?;

    test_various_storage(num_key, key_size, value_size).await?;

    Ok(())
}
