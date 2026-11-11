use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarU32};

const LENGTH_FIELD: usize = 4;

/// The opaque data of `Login`: a byte array that holds two parts, each behind an `i32le`
/// byte length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionRequest<'a> {
    /// UTF-8 JSON: an object with the key `chain`, an array of JWT strings.
    pub chain: &'a [u8],
    /// One JWT in its compact form.
    pub client_data: &'a [u8],
}

impl BedrockEncode for ConnectionRequest<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let total = (2 * LENGTH_FIELD)
            .checked_add(self.chain.len())
            .and_then(|len| len.checked_add(self.client_data.len()))
            .and_then(|len| u32::try_from(len).ok())
            .ok_or(Error::LengthOverflow)?;
        VarU32(total).encode(w)?;
        encode_part(self.chain, w)?;
        encode_part(self.client_data, w)
    }
}

impl<'a> BedrockDecode<'a> for ConnectionRequest<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let mut request: &'a [u8] = BedrockDecode::decode(r)?;
        let chain = decode_part(&mut request)?;
        let client_data = decode_part(&mut request)?;
        if !request.is_empty() {
            return Err(Error::TrailingBytes);
        }
        Ok(Self { chain, client_data })
    }
}

fn encode_part(part: &[u8], w: &mut Vec<u8>) -> Result<(), Error> {
    let len = i32::try_from(part.len()).map_err(|_| Error::LengthOverflow)?;
    len.encode(w)?;
    w.extend_from_slice(part);
    Ok(())
}

fn decode_part<'a>(r: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let len = usize::try_from(i32::decode(r)?).map_err(|_| Error::LengthOverflow)?;
    let Some((part, rest)) = r.split_at_checked(len) else {
        return Err(Error::UnexpectedEof);
    };
    *r = rest;
    Ok(part)
}
