use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{bail, Context, Result};

pub const SERVERDATA_AUTH: i32 = 3;
pub const SERVERDATA_AUTH_RESPONSE: i32 = 2;
pub const SERVERDATA_EXECCOMMAND: i32 = 2;
pub const SERVERDATA_RESPONSE_VALUE: i32 = 0;

#[derive(Debug, Clone)]
pub struct RconPacket {
    pub id: i32,
    pub pkt_type: i32,
    pub body: String,
}

impl RconPacket {
    /// Encode to wire format.
    pub fn encode(&self) -> Vec<u8> {
        let body = self.body.as_bytes();
        // length field = id(4) + type(4) + body + null(1) + padding_null(1)
        let length = (4 + 4 + body.len() + 2) as i32;
        let mut buf = Vec::with_capacity(4 + length as usize);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&self.id.to_le_bytes());
        buf.extend_from_slice(&self.pkt_type.to_le_bytes());
        buf.extend_from_slice(body);
        buf.push(0); // body null terminator
        buf.push(0); // packet null terminator
        buf
    }

    /// Decode from a complete raw buffer (including the leading 4-byte length).
    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 14 {
            bail!("RCON packet too short: {} bytes", buf.len());
        }
        let length = i32::from_le_bytes(buf[0..4].try_into().context("failed to read RCON length")?)
            as usize;
        if buf.len() < 4 + length {
            bail!("RCON buffer too short for declared length");
        }
        let id = i32::from_le_bytes(
            buf[4..8]
                .try_into()
                .context("failed to read RCON packet id")?,
        );
        let pkt_type = i32::from_le_bytes(
            buf[8..12]
                .try_into()
                .context("failed to read RCON packet type")?,
        );
        // body is between byte 12 and the null terminator
        let body_end = (4 + length).saturating_sub(2); // strip two trailing nulls
        let body_bytes = &buf[12..body_end.max(12)];
        let body = String::from_utf8_lossy(body_bytes).into_owned();
        Ok(Self { id, pkt_type, body })
    }
}

pub struct RconClient {
    stream: TcpStream,
    next_id: i32,
}

impl RconClient {
    /// Connect and authenticate. Errors if authentication fails.
    pub fn connect(host: &str, port: u16, password: &str) -> Result<Self> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect_timeout(
            &addr
                .parse()
                .context("invalid RCON address — expected host:port")?,
            Duration::from_secs(5),
        )
        .with_context(|| format!("cannot connect to RCON at {addr}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        let mut client = Self { stream, next_id: 1 };
        client.authenticate(password)?;
        Ok(client)
    }

    fn next_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send(&mut self, pkt: &RconPacket) -> Result<()> {
        self.stream.write_all(&pkt.encode())?;
        Ok(())
    }

    fn recv(&mut self) -> Result<RconPacket> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let length = i32::from_le_bytes(len_buf) as usize;
        let mut rest = vec![0u8; length];
        self.stream.read_exact(&mut rest)?;
        // Re-assemble with length prefix for decode
        let mut buf = Vec::with_capacity(4 + length);
        buf.extend_from_slice(&len_buf);
        buf.extend_from_slice(&rest);
        RconPacket::decode(&buf)
    }

    fn authenticate(&mut self, password: &str) -> Result<()> {
        let id = self.next_id();
        let auth_pkt = RconPacket {
            id,
            pkt_type: SERVERDATA_AUTH,
            body: password.to_string(),
        };
        self.send(&auth_pkt)?;
        // Source RCON sends TWO responses to auth:
        //   1. An empty SERVERDATA_RESPONSE_VALUE (type 0)
        //   2. The actual SERVERDATA_AUTH_RESPONSE (type 2) with id=-1 on failure
        let first = self.recv()?;
        // If the first response is already the auth response (some implementations skip the empty one)
        let auth_response = if first.pkt_type == SERVERDATA_AUTH_RESPONSE || first.id == -1 {
            first
        } else {
            // Consume the second packet which is the real auth response
            self.recv()?
        };
        if auth_response.id == -1 {
            bail!("RCON authentication failed — check rcon_password in safehouse.toml");
        }
        Ok(())
    }

    /// Send a command and return the response body.
    pub fn send_command(&mut self, command: &str) -> Result<String> {
        let id = self.next_id();
        let pkt = RconPacket {
            id,
            pkt_type: SERVERDATA_EXECCOMMAND,
            body: command.to_string(),
        };
        self.send(&pkt)?;
        let resp = self.recv()?;
        Ok(resp.body)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_encode_decode() {
        let pkt = RconPacket {
            id: 1,
            pkt_type: SERVERDATA_AUTH,
            body: "mypassword".to_string(),
        };
        let encoded = pkt.encode();
        let decoded = RconPacket::decode(&encoded).unwrap();
        assert_eq!(decoded.id, 1);
        assert_eq!(decoded.pkt_type, SERVERDATA_AUTH);
        assert_eq!(decoded.body, "mypassword");
    }

    #[test]
    fn test_empty_body_encode_decode() {
        let pkt = RconPacket {
            id: 42,
            pkt_type: SERVERDATA_EXECCOMMAND,
            body: String::new(),
        };
        let encoded = pkt.encode();
        let decoded = RconPacket::decode(&encoded).unwrap();
        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.pkt_type, SERVERDATA_EXECCOMMAND);
        assert_eq!(decoded.body, "");
    }
}
