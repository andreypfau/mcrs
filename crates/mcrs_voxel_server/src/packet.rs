use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;
use std::any::TypeId;
use std::sync::Arc;

/// A wire message's encoding, opaque to the engine. Deliberately not `Serialize`:
/// a game may generate its wire types separately from its domain types.
pub trait PacketCodec: Send + Sync + 'static {
    type Message;
    type Error: std::error::Error + Send + Sync + 'static;

    fn encode(&self, message: &Self::Message, out: &mut Vec<u8>) -> Result<(), Self::Error>;
    fn decode(&self, bytes: &[u8]) -> Result<Self::Message, Self::Error>;
}

/// What the registry stores: a codec with its message type erased, so the
/// engine can route bytes without naming any game type.
trait ErasedCodec: Send + Sync {
    fn encode_any(&self, message: &dyn std::any::Any, out: &mut Vec<u8>)
    -> Result<(), PacketError>;
    fn decode_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any + Send>, PacketError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error("no packet registered under id {0}")]
    Unregistered(u32),
    #[error("message did not match the codec registered for id {0}")]
    WrongMessage(u32),
    #[error("packet id {0} is already registered")]
    DuplicateId(u32),
    #[error(transparent)]
    Codec(#[from] Box<dyn std::error::Error + Send + Sync>),
}

struct Adapter<C>(C, u32);

impl<C> ErasedCodec for Adapter<C>
where
    C: PacketCodec,
    C::Message: Send + 'static,
{
    fn encode_any(
        &self,
        message: &dyn std::any::Any,
        out: &mut Vec<u8>,
    ) -> Result<(), PacketError> {
        let message = message
            .downcast_ref::<C::Message>()
            .ok_or(PacketError::WrongMessage(self.1))?;
        self.0
            .encode(message, out)
            .map_err(|e| PacketError::Codec(Box::new(e)))
    }

    fn decode_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any + Send>, PacketError> {
        self.0
            .decode(bytes)
            .map(|m| Box::new(m) as Box<dyn std::any::Any + Send>)
            .map_err(|e| PacketError::Codec(Box::new(e)))
    }
}

/// Wire messages a plugin has added. The reference engine closes this set at
/// build time; a second game cannot live with that, so it stays open here.
#[derive(Resource, Default)]
pub struct PacketRegistry {
    by_id: FxHashMap<u32, Arc<dyn ErasedCodec>>,
    ids: FxHashMap<TypeId, u32>,
}

impl PacketRegistry {
    pub fn register<C>(&mut self, id: u32, codec: C) -> Result<(), PacketError>
    where
        C: PacketCodec,
        C::Message: Send + 'static,
    {
        if self.by_id.contains_key(&id) {
            return Err(PacketError::DuplicateId(id));
        }
        self.by_id.insert(id, Arc::new(Adapter(codec, id)));
        self.ids.insert(TypeId::of::<C::Message>(), id);
        Ok(())
    }

    pub fn id_of<M: 'static>(&self) -> Option<u32> {
        self.ids.get(&TypeId::of::<M>()).copied()
    }

    pub fn encode<M: Send + 'static>(&self, message: &M) -> Result<(u32, Vec<u8>), PacketError> {
        let id = self
            .id_of::<M>()
            .ok_or(PacketError::Unregistered(u32::MAX))?;
        let codec = self.by_id.get(&id).ok_or(PacketError::Unregistered(id))?;
        let mut out = Vec::new();
        codec.encode_any(message, &mut out)?;
        Ok((id, out))
    }

    pub fn decode(
        &self,
        id: u32,
        bytes: &[u8],
    ) -> Result<Box<dyn std::any::Any + Send>, PacketError> {
        self.by_id
            .get(&id)
            .ok_or(PacketError::Unregistered(id))?
            .decode_any(bytes)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Ping(u16);

    #[derive(Debug, thiserror::Error)]
    #[error("short read")]
    struct Short;

    struct PingCodec;

    impl PacketCodec for PingCodec {
        type Message = Ping;
        type Error = Short;

        fn encode(&self, m: &Ping, out: &mut Vec<u8>) -> Result<(), Short> {
            out.extend_from_slice(&m.0.to_be_bytes());
            Ok(())
        }

        fn decode(&self, bytes: &[u8]) -> Result<Ping, Short> {
            let b: [u8; 2] = bytes.try_into().map_err(|_| Short)?;
            Ok(Ping(u16::from_be_bytes(b)))
        }
    }

    #[test]
    fn a_plugin_adds_a_wire_message_and_it_round_trips() {
        let mut reg = PacketRegistry::default();
        reg.register(7, PingCodec).unwrap();
        let (id, bytes) = reg.encode(&Ping(513)).unwrap();
        assert_eq!(id, 7);
        let back = reg.decode(id, &bytes).unwrap();
        assert_eq!(*back.downcast::<Ping>().unwrap(), Ping(513));
    }

    #[test]
    fn registering_one_id_twice_is_refused() {
        let mut reg = PacketRegistry::default();
        reg.register(7, PingCodec).unwrap();
        assert!(matches!(
            reg.register(7, PingCodec),
            Err(PacketError::DuplicateId(7))
        ));
    }

    #[test]
    fn an_unregistered_id_does_not_decode() {
        let reg = PacketRegistry::default();
        assert!(matches!(
            reg.decode(3, &[]),
            Err(PacketError::Unregistered(3))
        ));
    }
}
