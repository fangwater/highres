use anyhow::Context;
use clap::Parser;
use standalone_engine::engine::{orderbook_row, trade_row, Engine};
use standalone_engine::events::{InEvent, OutEvent};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value_t = 0)]
    sample_every: u64,

    #[arg(long)]
    summary_sid: Option<i32>,
}

fn print_out(ev: OutEvent) {
    println!("{}", serde_json::to_string(&ev).unwrap());
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    let mut engine: Option<Engine> = None;
    let mut n: u64 = 0;

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let ev: InEvent = serde_json::from_str(line).context("parse json event")?;
        n += 1;

        match ev {
            InEvent::Init { markets } => {
                engine = Some(Engine::new(markets));
                print_out(OutEvent::Ack {
                    ok: true,
                    msg: "init".to_string(),
                });
            }
            other => {
                let Some(engine) = engine.as_mut() else {
                    anyhow::bail!("need init first");
                };

                match other {
                    InEvent::Ob {
                        ts_us,
                        sid,
                        is_snapshot,
                        side,
                        price,
                        amount,
                    } => {
                        engine.on_orderbook(orderbook_row(ts_us, sid, is_snapshot, side, price, amount))?;
                    }
                    InEvent::Trade {
                        ts_us,
                        sid,
                        side,
                        price,
                        amount,
                    } => {
                        let fills = engine.on_trade(trade_row(ts_us, sid, side, price, amount))?;
                        for f in fills {
                            print_out(OutEvent::Fill {
                                ts_us: f.ts_us,
                                sid: f.sid,
                                client_order_id: f.client_order_id,
                                maker_side: f.maker_side,
                                price: f.price,
                                filled: f.filled,
                                remaining: f.remaining,
                                is_full: f.is_full,
                                pos: f.pos,
                                open: f.open,
                            });
                        }
                    }
                    InEvent::OwnNew {
                        ts_us,
                        sid,
                        client_order_id,
                        side,
                        price,
                        amount,
                        from_key,
                        dup_key,
                        target_sid,
                    } => {
                        let from_key = if from_key.is_empty() { "manual".to_string() } else { from_key };
                        let dup_key = if dup_key.is_empty() { "manual".to_string() } else { dup_key };
                        let target_sid = if target_sid == 0 { sid } else { target_sid };

                        engine.add_own_order(
                            ts_us,
                            sid,
                            client_order_id,
                            side,
                            price,
                            amount,
                            from_key,
                            dup_key,
                            target_sid,
                        )?;
                    }
                    InEvent::OwnCancel { client_order_id } => {
                        let ok = engine.cancel_own_order(&client_order_id)?;
                        print_out(OutEvent::Ack {
                            ok,
                            msg: format!("cancel {}", client_order_id),
                        });
                    }
                    InEvent::Tick { ts_ms } => {
                        engine.on_tick(ts_ms);
                    }
                    InEvent::Sample {} => {
                        let sid = args.summary_sid.unwrap_or_else(|| {
                            *engine
                                .state
                                .markets
                                .keys()
                                .min()
                                .unwrap_or(&0)
                        });
                        if let Some(s) = engine.summary(sid) {
                            print_out(OutEvent::Summary {
                                sid: s.sid,
                                is_finish_snap: s.is_finish_snap,
                                bid1: s.bid1,
                                ask1: s.ask1,
                                pending_bids: s.pending_bids,
                                pending_asks: s.pending_asks,
                                pos: s.pos,
                                open: s.open,
                            });
                        }
                    }
                    InEvent::Clear {} => {
                        engine.clear();
                        print_out(OutEvent::Ack {
                            ok: true,
                            msg: "clear".to_string(),
                        });
                    }
                    InEvent::Init { .. } => unreachable!(),
                }
            }
        }

        if args.sample_every > 0 && (n % args.sample_every) == 0 {
            if let Some(engine) = engine.as_ref() {
                let sid = args.summary_sid.unwrap_or_else(|| {
                    *engine.state.markets.keys().min().unwrap_or(&0)
                });
                if let Some(s) = engine.summary(sid) {
                    print_out(OutEvent::Summary {
                        sid: s.sid,
                        is_finish_snap: s.is_finish_snap,
                        bid1: s.bid1,
                        ask1: s.ask1,
                        pending_bids: s.pending_bids,
                        pending_asks: s.pending_asks,
                        pos: s.pos,
                        open: s.open,
                    });
                }
            }
        }
    }

    Ok(())
}
