use crate::client;
use crate::client::{Event, Login, Order, Signal};
use crate::{
    Action, ConnectionState, Disconnect, Observed, PlayerBan, Scenario, StepTrace, Trace,
    TraceVersion,
};
use anyhow::Context;
use mcrs_minecraft_core::VERSION;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time;

// A command and a leave wait up to `FIRST_EVENT` for the first chat message, disconnect or close,
// so that a slow server does not end the step before its answer.
const FIRST_EVENT: Duration = Duration::from_secs(10);
const JOIN_LIMIT: Duration = Duration::from_secs(60);

struct Clients {
    quiet: Duration,
    orders: BTreeMap<String, UnboundedSender<Order>>,
    events_in: UnboundedSender<Event>,
    events: UnboundedReceiver<Event>,
}

/// Runs the steps of `scenario` with clients that connect to `address`, and returns what they
/// observed. A step is complete when no client reported a chat message, a disconnect or a close
/// for `quiet`. `read_player_bans` returns the player ban file of the target as it is now.
///
/// Does not apply `scenario.target`: the caller does that before the call.
pub async fn run<B>(
    scenario: &Scenario,
    address: SocketAddr,
    quiet: Duration,
    mut read_player_bans: B,
) -> anyhow::Result<Trace>
where
    B: FnMut() -> anyhow::Result<Vec<PlayerBan>>,
{
    let (events_in, events) = mpsc::unbounded_channel();
    let mut clients = Clients {
        quiet,
        orders: BTreeMap::new(),
        events_in,
        events,
    };
    let mut steps = Vec::with_capacity(scenario.steps.len());
    for step in &scenario.steps {
        let observed = match &step.action {
            Action::Join(name) => clients.join(address, name).await,
            Action::Leave(name) => clients.leave(name).await,
            Action::Command { client, line } => clients.command(client, line).await,
        }
        .with_context(|| format!("step {}", step.id))?;
        let player_bans = if step.read_player_bans {
            let mut bans = read_player_bans()?;
            bans.sort_by(|left, right| left.name.cmp(&right.name));
            Some(bans)
        } else {
            None
        };
        steps.push(StepTrace {
            id: step.id.clone(),
            observed,
            player_bans,
        });
    }
    Ok(Trace {
        version: TraceVersion {
            name: VERSION.name.to_owned(),
            protocol: VERSION.protocol_version,
        },
        steps,
    })
}

impl Clients {
    async fn join(&mut self, address: SocketAddr, name: &str) -> anyhow::Result<Observed> {
        anyhow::ensure!(
            !self.orders.contains_key(name),
            "{name} is already connected"
        );
        let login = time::timeout(JOIN_LIMIT, client::log_in(address, name))
            .await
            .with_context(|| format!("{name} did not join in {JOIN_LIMIT:?}"))??;
        let io = match login {
            Login::Joined(io) => io,
            Login::Refused { state, reason } => return Ok(Observed::Refused { state, reason }),
        };
        let (orders_in, orders) = mpsc::unbounded_channel();
        self.orders.insert(name.to_owned(), orders_in);
        tokio::spawn(client::session(
            *io,
            name.to_owned(),
            orders,
            self.events_in.clone(),
        ));
        for event in self.settle(self.quiet).await {
            if event.client != name {
                continue;
            }
            let reason = match event.signal {
                Signal::Chat(_) => continue,
                Signal::Disconnected(reason) => Some(reason),
                Signal::Closed => None,
            };
            return Ok(Observed::Refused {
                state: ConnectionState::Play,
                reason,
            });
        }
        Ok(Observed::Joined)
    }

    async fn leave(&mut self, name: &str) -> anyhow::Result<Observed> {
        self.order(name, Order::Leave)?;
        let closed = self
            .settle(FIRST_EVENT)
            .await
            .iter()
            .any(|event| event.client == name && !matches!(event.signal, Signal::Chat(_)));
        anyhow::ensure!(closed, "the connection of {name} did not close");
        Ok(Observed::Left)
    }

    async fn command(&mut self, sender: &str, line: &str) -> anyhow::Result<Observed> {
        self.order(sender, Order::Command(line.to_owned()))?;
        let mut replies = Vec::new();
        let mut disconnects = Vec::new();
        for event in self.settle(FIRST_EVENT).await {
            match event.signal {
                Signal::Chat(text) if event.client == sender => replies.push(text),
                Signal::Disconnected(reason) => disconnects.push(Disconnect {
                    client: event.client,
                    reason,
                }),
                Signal::Chat(_) | Signal::Closed => {}
            }
        }
        disconnects.sort_by(|left, right| left.client.cmp(&right.client));
        Ok(Observed::Command {
            replies,
            disconnects,
        })
    }

    fn order(&self, name: &str, order: Order) -> anyhow::Result<()> {
        let orders = self
            .orders
            .get(name)
            .with_context(|| format!("{name} is not connected"))?;
        orders
            .send(order)
            .ok()
            .with_context(|| format!("the connection of {name} ended"))
    }

    /// Collects the events until none arrives for `quiet`; the first has `first_limit` to arrive.
    async fn settle(&mut self, first_limit: Duration) -> Vec<Event> {
        let mut seen = Vec::new();
        let mut limit = first_limit;
        while let Ok(Some(event)) = time::timeout(limit, self.events.recv()).await {
            if !matches!(event.signal, Signal::Chat(_)) {
                self.orders.remove(&event.client);
            }
            seen.push(event);
            limit = self.quiet;
        }
        seen
    }
}
