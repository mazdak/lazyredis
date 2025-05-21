use anyhow::Result;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection};

pub async fn seed_redis_data(redis_url: &str, db_index: u8) -> Result<()> {
    println!("Connecting to {} (DB {}) to seed data...", redis_url, db_index);
    let client = Client::open(redis_url)?;
    let mut con: MultiplexedConnection = client.get_multiplexed_async_connection().await?;

    redis::cmd("SELECT").arg(db_index).query_async::<()>(&mut con).await?;
    println!("Selected database {}.", db_index);

    println!("Flushing database {}...", db_index);
    redis::cmd("FLUSHDB").query_async::<()>(&mut con).await?;
    println!("Database {} flushed.", db_index);

    println!("Seeding a large volume of keys...");

    for i in 0..1000 {
        let _: () = con.set(format!("seed:simple:{}", i), format!("Simple value {}", i)).await?;
    }
    if 1000 % 100 == 0 { println!("Seeded 1000 simple keys...");}

    for i in 0..50 {
        for j in 0..20 {
            for k in 0..10 {
                let key = format!("seed:level1:{}:level2:{}:key:{}", i, j, k);
                let _: () = con.set(&key, format!("Value for {}", key)).await?;
            }
        }
        if (i+1) % 10 == 0 { println!("Seeded hierarchy for level1 up to {}...", i+1); }
    }
    println!("Seeded nested keys (50*20*10 = 10,000 keys).");

    for i in 0..100 {
        let _: () = con.set(format!("seed/path/num_{}", i), format!("Path value {}", i)).await?;
        let _: () = con.set(format!("seed.dot.num_{}", i), format!("Dot value {}", i)).await?;
        let _: () = con.set(format!("seed-dash-num_{}", i), format!("Dash value {}", i)).await?;
    }
    println!("Seeded 300 keys with various delimiters.");

    for i in 0..50 {
        let mut fields = Vec::new();
        for j in 0..200 {
            fields.push((format!("field_{}", j), format!("value_for_hash_{}_field_{}", i, j)));
        }
        let _: () = con.hset_multiple(format!("seed:large_hash:{}", i), &fields).await?;
        if (i+1) % 10 == 0 { println!("Seeded large hash {}...", i+1); }
    }
    println!("Seeded 50 large hashes (50 * 200 fields).");

    for i in 0..50 {
        let mut items = Vec::new();
        for j in 0..500 {
            items.push(format!("list_{}_item_{}", i, j));
        }
        let _: () = con.rpush(format!("seed:large_list:{}", i), items).await?;
        if (i+1) % 10 == 0 { println!("Seeded large list {}...", i+1); }
    }
    println!("Seeded 50 large lists (50 * 500 items).");
    
    for i in 0..50 {
        let mut members = Vec::new();
        for j in 0..300 {
            members.push(format!("set_{}_member_{}", i, j));
        }
        let _: () = con.sadd(format!("seed:large_set:{}", i), members).await?;
         if (i+1) % 10 == 0 { println!("Seeded large set {}...", i+1); }
    }
    println!("Seeded 50 large sets (50 * 300 members).");

    for i in 0..50 {
        let mut members_scores = Vec::new();
        for j in 0..400 {
            members_scores.push(((j * 10) as f64, format!("zset_{}_member_{}", i, j)));
        }
        let _: () = con.zadd_multiple(format!("seed:large_zset:{}", i), &members_scores).await?;
        if (i+1) % 10 == 0 { println!("Seeded large zset {}...", i+1); }
    }
    println!("Seeded 50 large zsets (50 * 400 members/scores).");

    for i in 0..10 {
        for j in 0..1000 {
            let _: String = con.xadd(format!("seed:large_stream:{}", i), "*", &[
                ("event_id", format!("{}-{}", i, j)),
                ("sensor_id", format!("sensor_{}", i % 5)),
                ("timestamp", (j * 1000).to_string()),
                ("payload", format!("Some data payload for event {}-{}, could be JSON or any string.", i,j))
            ]).await?;
        }
        println!("Seeded stream seed:large_stream:{} with 1000 entries.", i);
    }
    println!("Seeded 10 streams with 1000 entries each.");
    
    println!("Seeding original specific test keys...");
    let _: () = con.set("seed:string", "Hello from LazyRedis Seeder!").await?;
    let _: () = con.set("seed:another_string", "This string is a bit longer and might require scrolling to see fully in the value panel if it is narrow enough.").await?;
    let _: () = con.hset_multiple("seed:hash", &[("field1", "Value1"), ("field2", "Another Value"), ("long_field_name_for_testing_wrapping", "This value is also quite long to test how wrapping behaves in the TUI for hash values.")]).await?;
    let _: () = con.rpush("seed:list", &["Item 1", "Item 2", "Item 3", "Yet another item", "And one more for good measure"]).await?;
    let _: () = con.sadd("seed:set", &["MemberA", "MemberB", "MemberC", "MemberD", "MemberE", "MemberA"]).await?;
    let _: () = con.zadd_multiple("seed:zset", &[ (10.0, "Ten"), (1.0, "One"), (30.0, "Thirty"), (20.0, "Twenty"), (5.0, "Five"), (100.0, "One Hundred"), (15.0, "Fifteen")]).await?;
    let _: String = con.xadd("seed:stream", "*", &[("fieldA", "valueA1"), ("fieldB", "valueB1")]).await?;
    let _: String = con.xadd("seed:stream", "*", &[("sensor-id", "1234"), ("temperature", "19.8")]).await?;
    let _: String = con.xadd("seed:stream", "*", &[("message", "Hello World"), ("user", "Alice"), ("timestamp", "1678886400000")]).await?;
    println!("Seeding empty types for testing views...");
    let _: () = con.hset("seed:empty_hash", "placeholder_field", "placeholder_value").await?;
    let _: i32 = con.hdel("seed:empty_hash", "placeholder_field").await?;
    let _: () = con.rpush("seed:empty_list", "placeholder").await?;
    let _: String = con.lpop::<_, String>("seed:empty_list", Default::default()).await?;
    let _: () = con.sadd("seed:empty_set", "placeholder").await?;
    let _: i32 = con.srem("seed:empty_set", "placeholder").await?;
    let _: () = con.zadd("seed:empty_zset", "placeholder", 1.0).await?;
    let _: i32 = con.zrem("seed:empty_zset", "placeholder").await?;

    println!("Finished seeding data.");
    Ok(())
}