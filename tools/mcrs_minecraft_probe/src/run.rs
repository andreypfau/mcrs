use crate::client;
use crate::client::{Event, Login, Order, Signal};
use crate::{
    Action, ConnectionState, Disconnect, Host, MenuEvent, Observe, Observed, Scenario, StepTrace,
    Trace, TraceVersion,
};
use anyhow::Context;
use mcrs_minecraft_core::VERSION;
use std::collections::BTreeMap;
use std::mem;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time;

// A command and a leave wait up to `FIRST_EVENT` for the first chat message, disconnect or close,
// so that a slow server does not end the step before its answer.
const FIRST_EVENT: Duration = Duration::from_secs(10);
// The server answers a game packet in the next ticks or not at all, so a silent step ends sooner.
const FIRST_PACKET: Duration = Duration::from_secs(2);
const JOIN_LIMIT: Duration = Duration::from_secs(60);

struct Clients {
    quiet: Duration,
    observe: Observe,
    menu: Vec<MenuEvent>,
    orders: BTreeMap<String, UnboundedSender<Order>>,
    events_in: UnboundedSender<Event>,
    events: UnboundedReceiver<Event>,
}

/// Runs the steps of `scenario` with clients that connect to `address`, and returns what they
/// observed. A step is complete when no client reported a chat message, a menu packet, a
/// disconnect or a close for `quiet`. `host` reads the files of the target and restarts it.
///
/// Does not apply `scenario.target`: the caller does that before the call.
pub async fn run<H: Host>(
    scenario: &Scenario,
    address: SocketAddr,
    quiet: Duration,
    host: &mut H,
) -> anyhow::Result<Trace> {
    let (events_in, events) = mpsc::unbounded_channel();
    let mut clients = Clients {
        quiet,
        observe: scenario.observe,
        menu: Vec::new(),
        orders: BTreeMap::new(),
        events_in,
        events,
    };
    let mut steps = Vec::with_capacity(scenario.steps.len());
    for step in &scenario.steps {
        let observed = clients
            .act(&step.action, address, host)
            .await
            .with_context(|| format!("step {}", step.id))?;
        let player_bans = if step.read_player_bans {
            let mut bans = host.read_player_bans()?;
            bans.sort_by(|left, right| left.name.cmp(&right.name));
            Some(bans)
        } else {
            None
        };
        let player_file = match &step.read_player_file {
            Some(read) => {
                let file = host.read_player_file(&read.client).with_context(|| {
                    format!("step {}: the player file of {}", step.id, read.client)
                })?;
                let fields = read
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.clone(),
                            file.get(field).map(mcrs_minecraft_snbt::write),
                        )
                    })
                    .collect();
                Some(fields)
            }
            None => None,
        };
        let menu = mem::take(&mut clients.menu);
        steps.push(StepTrace {
            id: step.id.clone(),
            observed,
            player_bans,
            inventory: scenario.observe.inventory.then_some(menu),
            player_file,
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
    async fn act<H: Host>(
        &mut self,
        action: &Action,
        address: SocketAddr,
        host: &mut H,
    ) -> anyhow::Result<Observed> {
        match action {
            Action::Join(name) => self.join(address, name).await,
            Action::Leave(name) => self.leave(name).await,
            Action::Command { client, line } => self.command(client, line).await,
            Action::Click(click) => self.send(&click.client, Order::Click(click.clone())).await,
            Action::Close { client, container } => {
                self.send(client, Order::Close(*container)).await
            }
            Action::SelectSlot { client, slot } => {
                self.send(client, Order::SelectSlot(*slot)).await
            }
            Action::PlayerAction { client, status } => {
                self.send(client, Order::PlayerAction((*status).into()))
                    .await
            }
            Action::CreativeSlot {
                client,
                slot,
                stack,
            } => {
                let order = Order::CreativeSlot {
                    slot: *slot,
                    stack: *stack,
                };
                self.send(client, order).await
            }
            Action::UseBlock { client, position } => {
                self.send(client, Order::UseBlock(*position)).await
            }
            Action::RestartServer {} => self.restart(host).await,
        }
    }

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
            self.observe,
            orders,
            self.events_in.clone(),
        ));
        let first_limit = if self.observe.inventory {
            FIRST_PACKET
        } else {
            self.quiet
        };
        for event in self.settle(first_limit).await {
            if event.client != name {
                continue;
            }
            let reason = match event.signal {
                Signal::Chat(_) | Signal::Menu(_) => continue,
                Signal::Disconnected(reason) => Some(reason),
                Signal::Closed | Signal::Failed(_) => None,
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
        let closed = self.settle(FIRST_EVENT).await.iter().any(|event| {
            event.client == name
                && matches!(
                    event.signal,
                    Signal::Disconnected(_) | Signal::Closed | Signal::Failed(_)
                )
        });
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
                Signal::Chat(_) | Signal::Menu(_) | Signal::Closed | Signal::Failed(_) => {}
            }
        }
        disconnects.sort_by(|left, right| left.client.cmp(&right.client));
        Ok(Observed::Command {
            replies,
            disconnects,
        })
    }

    async fn send(&mut self, sender: &str, order: Order) -> anyhow::Result<Observed> {
        self.order(sender, order)?;
        let mut disconnects = Vec::new();
        for event in self.settle(FIRST_PACKET).await {
            match event.signal {
                Signal::Disconnected(reason) => disconnects.push(Disconnect {
                    client: event.client,
                    reason,
                }),
                Signal::Closed if event.client == sender => {
                    anyhow::bail!("the connection of {sender} closed without a disconnect packet")
                }
                Signal::Failed(error) if event.client == sender => {
                    anyhow::bail!("the connection of {sender} closed after an error: {error}")
                }
                Signal::Chat(_) | Signal::Menu(_) | Signal::Closed | Signal::Failed(_) => {}
            }
        }
        disconnects.sort_by(|left, right| left.client.cmp(&right.client));
        Ok(Observed::Sent { disconnects })
    }

    async fn restart<H: Host>(&mut self, host: &mut H) -> anyhow::Result<Observed> {
        if !self.orders.is_empty() {
            self.orders.clear();
            self.settle(FIRST_EVENT).await;
        }
        host.restart_server()?;
        Ok(Observed::Restarted)
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
    /// Keeps the menu packets aside for the trace of the step and returns the other events.
    async fn settle(&mut self, first_limit: Duration) -> Vec<Event> {
        let mut seen = Vec::new();
        let mut limit = first_limit;
        while let Ok(Some(event)) = time::timeout(limit, self.events.recv()).await {
            limit = self.quiet;
            match event.signal {
                Signal::Menu(packet) => self.menu.push(MenuEvent {
                    client: event.client,
                    packet,
                }),
                Signal::Chat(_) => seen.push(event),
                Signal::Disconnected(_) | Signal::Closed | Signal::Failed(_) => {
                    self.orders.remove(&event.client);
                    seen.push(event);
                }
            }
        }
        seen
    }
}
