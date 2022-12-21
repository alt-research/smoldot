// Smoldot
// Copyright (C) 2019-2022  Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use clap::{Command, Arg};
use serde_json::{Value,Number, Map};

use core::iter;

fn build_chain_spec(genesis_path: String, boot_nodes: String) -> String {
    let genesis_datas = std::fs::read_to_string(std::path::Path::new(&genesis_path))
    .expect("genesis_path read failed!");

    let mut v: Map<String, Value> = serde_json::from_str(genesis_datas.as_str()).expect("parse genesis json failed!");

    v.insert("bootNodes".to_string(), Value::Array(vec![Value::String(boot_nodes)]));

    Value::Object(v).to_string()
}

fn main() {
    // The `smoldot_light` library uses the `log` crate to emit logs.
    // We need to register some kind of logs listener, in this example `env_logger`.
    // See also <https://docs.rs/log>.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let matches = Command::new("light node bin test")
        .version("0.1.0")
        .arg(
            Arg::new("genesis")
                .short('g')
                .long("genesis")
                .help("genesis path"),
        )        
        .arg(
            Arg::new("bootnode")
                .short('b')
                .long("bootnode")
                .help("bootnode for sync"),
        )
        .get_matches();

    let genesis_path = matches.get_one::<String>("genesis").cloned().expect("no genesis path");
    log::info!("genesis from {:?}", genesis_path);

    let boot_nodes = matches.get_one::<String>("bootnode").cloned().expect("no bootnode");
    log::info!("boot_nodes from {:?}", genesis_path);

    let chain_spec = build_chain_spec(genesis_path, boot_nodes);

    // Now initialize the client. This does nothing except allocate resources.
    // The `Client` struct requires a generic parameter that provides platform bindings. In this
    // example, we provide `AsyncStdTcpWebSocket`, which are the "plug and play" default platform.
    // Any advance usage, such as embedding a client in WebAssembly, will likely require a custom
    // implementation of these bindings.
    let mut client = smoldot_light::Client::<
        smoldot_light::platform::async_std::AsyncStdTcpWebSocket,
    >::new(smoldot_light::ClientConfig {
        // The smoldot client will need to spawn tasks that run in the background. In order to do
        // so, we need to provide a "tasks spawner".
        tasks_spawner: Box::new(move |_name, task| {
            async_std::task::spawn(task);
        }),
        system_name: env!("CARGO_PKG_NAME").into(),
        system_version: env!("CARGO_PKG_VERSION").into(),
    });

    // Ask the client to connect to a chain.
    let smoldot_light::AddChainSuccess {
        chain_id,
        json_rpc_responses,
        ..
    } = client
        .add_chain(smoldot_light::AddChainConfig {
            // The most important field of the configuration is the chain specification. This is a
            // JSON document containing all the information necessary for the client to connect to said
            // chain.
            specification: chain_spec.as_str(),

            // This field is necessary only if adding a parachain.
            potential_relay_chains: iter::empty(),

            // After a chain has been added, it is possible to extract a "database" (in the form of a
            // simple string). This database can later be passed back the next time the same chain is
            // added again.
            // A database with an invalid format is simply ignored by the client.
            // In this example, we don't use this feature, and as such we simply pass an empty string,
            // which is intentionally an invalid database content.
            database_content: "",

            // The client gives the possibility to insert an opaque "user data" alongside each chain.
            // This avoids having to create a separate `HashMap<ChainId, ...>` in parallel of the
            // client.
            // In this example, this feature isn't used. The chain simply has `()`.
            user_data: (),

            disable_json_rpc: false,
        })
        .unwrap();

    let mut json_rpc_responses = json_rpc_responses.expect("");

    // Send a JSON-RPC request to the chain.
    // The example here asks the client to send us notifications whenever the new best block has
    // changed.
    // Calling this function only queues the request. It is not processed immediately.
    // An `Err` is returned immediately if and only if the request isn't a proper JSON-RPC request
    // or if the channel of JSON-RPC responses is clogged.
    client
        .json_rpc_request(
            r#"{"id":1,"jsonrpc":"2.0","method":"chain_subscribeNewHeads","params":[]}"#,
            chain_id,
        )
        .unwrap();

    client
        .json_rpc_request(
            r#"{"id":2,"jsonrpc":"2.0","method":"grandpa_subscribeJustifications","params":[]}"#,
            chain_id,
        )
        .unwrap();

    // Now block the execution forever and print the responses received on the channel of
    // JSON-RPC responses.
    async_std::task::spawn(async move {
        loop {
            let response = json_rpc_responses.next().await.unwrap();
            println!("JSON-RPC response: {}", response);
        }
    });

    async_std::task::block_on(async move {
        let mut id = 3; // last req is 2,
        let mut req = Map::<String, Value>::new();
        req.insert("jsonrpc".to_string(), Value::String("2.0".to_string()));
        req.insert("method".to_string(), Value::String("system_health".to_string()));
        req.insert("params".to_string(), Value::Array(vec![]));

        loop {
            req.insert("id".to_string(), Value::Number(Number::from(id)));
            id = id + 1;
            let req_string = Value::Object(req.clone()).to_string();

            println!("JSON-RPC health req: {:?}", req_string);

            let response =     client
            .json_rpc_request(
                req_string,
                chain_id,
            );
            println!("JSON-RPC health response: {:?}", response);

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }
    })
}
