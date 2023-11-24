use std::process;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use bytes::Bytes;
use dotenv::dotenv;
use ethers::core::{
    k256::ecdsa::SigningKey,
    rand::Rng,
};
use ethers::prelude::*;
use ethers::utils::parse_units;
use log::{error, info, warn};
use rayon::prelude::*;
use serde::Deserialize;
use tokio;

use crate::initialization::{print_banner, setup_logger};

mod initialization;

static TIMES: AtomicUsize = AtomicUsize::new(0);
static NONCE: AtomicUsize = AtomicUsize::new(0);
static SUCCESS: AtomicU32 = AtomicU32::new(0);
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub rpc_url: String,
    pub private_key: String,
    pub tick: String,
    pub amt: String,
    pub difficulty: String,
    pub count: u32,
    pub max_fee_per_gas: u32,
    pub max_priority_fee_per_gas: u32,
}

impl Config {
    pub fn get_random_data(&self) -> String {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let nonce = rand::thread_rng().gen_range(10_000_000_000_000..1_000_000_000_000_000_000);
        let nonce = timestamp + nonce;
        let date = format!("data:application/json,{{\"p\":\"ierc-20\",\"op\":\"mint\",\"tick\":\"{}\",\"amt\":\"{}\",\"nonce\":\"{}\"}}", self.tick, self.amt, nonce);
        date
    }
}

pub struct GasPrice {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    setup_logger()?;
    print_banner();

    info!("开始执行任务");
    warn!("Author:[𝕏] @0xNaiXi");
    warn!("Author:[𝕏] @0xNaiXi");
    warn!("Author:[𝕏] @0xNaiXi");
    // 解析 .env 文件
    let config = envy::from_env::<Config>()?;
    let provider = Provider::<Http>::try_from(&config.rpc_url)?;
    let chain_id = provider.get_chainid().await?;
    let private_key = config.private_key.clone();
    let wallet = private_key.parse::<LocalWallet>().unwrap().with_chain_id(chain_id.as_u64());
    let address = wallet.address();
    let nonce = provider.get_transaction_count(address, None).await?;
    NONCE.fetch_add(nonce.as_usize(), Ordering::Relaxed);
    info!("当前链ID: {}", chain_id);
    info!("钱包nonce: {}", NONCE.load(Ordering::Relaxed));
    let pu = parse_units(config.max_fee_per_gas, "gwei").unwrap();
    let max_fee_per_gas = U256::from(pu);
    let pu = parse_units(config.max_priority_fee_per_gas, "gwei").unwrap();
    let max_priority_fee_per_gas = U256::from(pu);
    let gas_price = GasPrice {
        max_fee_per_gas,
        max_priority_fee_per_gas,
    };

    tokio::spawn(async move {
        loop {
            let last_times = TIMES.load(Ordering::Relaxed) as u64;
            tokio::time::sleep(Duration::new(10, 0)).await;
            let rate = (TIMES.load(Ordering::Relaxed) as u64 - last_times) / 10;
            warn!("计算hash总次数 {}  速率 {} hashes/s", TIMES.load(Ordering::Relaxed), rate);
        }
    });

    rayon::iter::repeat(())
        .for_each(|_| {
            make_tx(&provider, &wallet, &config, &gas_price).unwrap();
        });
    Ok(())
}

fn make_tx(provider: &Provider<Http>, wallet: &Wallet<SigningKey>, config: &Config, gas_price: &GasPrice) -> Result<bool, Box<dyn std::error::Error>> {
    let chain_id = wallet.chain_id();
    let nonce = U256::from(NONCE.load(Ordering::Relaxed));
    let data = config.get_random_data();

    // println!("data: {}", data);
    let data = Bytes::from(data);
    let tx = Eip1559TransactionRequest::new()
        .chain_id(chain_id)
        .from(wallet.address())
        .to(ZERO_ADDRESS)
        .value(0)
        .max_fee_per_gas(gas_price.max_fee_per_gas)
        .max_priority_fee_per_gas(gas_price.max_priority_fee_per_gas)
        .gas(50000)
        .nonce(nonce)
        .data(data)
        .access_list(vec![])
        .into();
    let signature = wallet.sign_transaction_sync(&tx)?;
    let hash = tx.hash(&signature);
    TIMES.fetch_add(1, Ordering::Relaxed);
    //println!("hash: {:?}", hash);
    let hash = format!("{:?}", hash);
    let flag = hash.starts_with(&config.difficulty);
    if flag {
        let signed_tx = tx.rlp_signed(&signature);
        if nonce.as_usize().eq(&NONCE.load(Ordering::Relaxed)) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let tx_hash = rt.block_on(provider.send_raw_transaction(signed_tx));
            match tx_hash {
                Ok(tx_hash) => {
                    info!("交易发送成功: {:?}", tx_hash.tx_hash());
                    NONCE.fetch_add(1, Ordering::Relaxed);
                    SUCCESS.fetch_add(1, Ordering::Relaxed);
                    if config.count.eq(&SUCCESS.load(Ordering::Relaxed)) {
                        // 任务执行完毕 info 设置绿色
                        info!("任务执行完毕");
                        process::exit(0);
                    }
                }
                Err(e) => {
                    error!("交易发送失败: {:?}", e);
                }
            }
            return Ok(flag);
        }
    }
    Ok(flag)
}