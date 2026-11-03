//! Non-blocking UDP peer-to-peer & loopback network synchronization (`PLAYER_ONE` / `PLAYER_TWO`).

use crate::Result;
use std::net::{SocketAddr, UdpSocket};

pub const MAGIC: &[u8; 4] = b"BW2N";
pub const PACKET_SIZE: usize = 40;

pub const BTN_FIRE: u16 = 1 << 0;
pub const BTN_LOCK: u16 = 1 << 1;
pub const BTN_CALL: u16 = 1 << 2;
pub const BTN_SEND: u16 = 1 << 3;
pub const BTN_DISMISS: u16 = 1 << 4;
pub const BTN_TRANSFER: u16 = 1 << 5;
pub const BTN_CONFIRM: u16 = 1 << 6;
pub const BTN_SKIP_CUTSCENE: u16 = 1 << 7;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInputPacket {
    pub frame: u64,
    pub player_slot: u8,
    pub buttons: u16,
    pub movement: [f32; 2],
    pub aim_yaw_delta: f32,
    pub aim_pitch_delta: f32,
    pub score_p1: f32,
    pub score_p2: f32,
}

impl PlayerInputPacket {
    pub fn to_bytes(&self) -> [u8; PACKET_SIZE] {
        let mut out = [0u8; PACKET_SIZE];
        out[0..4].copy_from_slice(MAGIC);
        out[4..12].copy_from_slice(&self.frame.to_le_bytes());
        out[12] = self.player_slot;
        out[13..15].copy_from_slice(&self.buttons.to_le_bytes());
        out[16..20].copy_from_slice(&self.movement[0].to_le_bytes());
        out[20..24].copy_from_slice(&self.movement[1].to_le_bytes());
        out[24..28].copy_from_slice(&self.aim_yaw_delta.to_le_bytes());
        out[28..32].copy_from_slice(&self.aim_pitch_delta.to_le_bytes());
        out[32..36].copy_from_slice(&self.score_p1.to_le_bytes());
        out[36..40].copy_from_slice(&self.score_p2.to_le_bytes());
        let mut sum = 0u8;
        for &b in &out[0..15] {
            sum = sum.wrapping_add(b);
        }
        for &b in &out[16..40] {
            sum = sum.wrapping_add(b);
        }
        out[15] = sum;
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < PACKET_SIZE {
            return Err("Truncated network packet".into());
        }
        if &bytes[0..4] != MAGIC {
            return Err("Invalid network packet magic".into());
        }
        let mut sum = 0u8;
        for &b in &bytes[0..15] {
            sum = sum.wrapping_add(b);
        }
        for &b in &bytes[16..40] {
            sum = sum.wrapping_add(b);
        }
        if bytes[15] != sum {
            return Err("Network packet checksum mismatch".into());
        }
        let frame = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
        let player_slot = bytes[12];
        let buttons = u16::from_le_bytes(bytes[13..15].try_into().unwrap());
        let mx = f32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let my = f32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let yaw = f32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let pitch = f32::from_le_bytes(bytes[28..32].try_into().unwrap());
        let score_p1 = f32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let score_p2 = f32::from_le_bytes(bytes[36..40].try_into().unwrap());
        if [mx, my, yaw, pitch, score_p1, score_p2]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err("Non-finite float in network packet".into());
        }
        Ok(Self {
            frame,
            player_slot,
            buttons,
            movement: [mx, my],
            aim_yaw_delta: yaw,
            aim_pitch_delta: pitch,
            score_p1,
            score_p2,
        })
    }
}

pub struct NetPeer {
    socket: UdpSocket,
    remote: SocketAddr,
    pub local_slot: u8,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub last_remote_packet: Option<PlayerInputPacket>,
}

impl NetPeer {
    pub fn bind(bind_addr: &str, remote_addr: &str, local_slot: u8) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).map_err(|e| format!("UDP bind failed: {e}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("UDP nonblocking failed: {e}"))?;
        let remote: SocketAddr = remote_addr
            .parse()
            .map_err(|e| format!("Invalid remote address: {e}"))?;
        Ok(Self {
            socket,
            remote,
            local_slot,
            packets_sent: 0,
            packets_received: 0,
            last_remote_packet: None,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr().map_err(|e| e.to_string())
    }

    pub fn set_remote(&mut self, remote: SocketAddr) {
        self.remote = remote;
    }

    pub fn send_packet(&mut self, pkt: &PlayerInputPacket) -> Result<()> {
        let raw = pkt.to_bytes();
        let _ = self
            .socket
            .send_to(&raw, self.remote)
            .map_err(|e| e.to_string())?;
        self.packets_sent += 1;
        Ok(())
    }

    pub fn poll_packets(&mut self) -> Vec<PlayerInputPacket> {
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while let Ok((len, _src)) = self.socket.recv_from(&mut buf) {
            if let Ok(pkt) = PlayerInputPacket::from_bytes(&buf[..len]) {
                self.packets_received += 1;
                self.last_remote_packet = Some(pkt);
                out.push(pkt);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_peer_to_peer_exchanges_player_one_and_player_two_frames() {
        let mut host = NetPeer::bind("127.0.0.1:0", "127.0.0.1:9", 1).unwrap();
        let mut client = NetPeer::bind("127.0.0.1:0", "127.0.0.1:9", 2).unwrap();
        let host_addr = host.local_addr().unwrap();
        let client_addr = client.local_addr().unwrap();
        host.set_remote(client_addr);
        client.set_remote(host_addr);

        let p1_pkt = PlayerInputPacket {
            frame: 42,
            player_slot: 1,
            buttons: BTN_FIRE | BTN_LOCK,
            movement: [0.0, 1.0],
            aim_yaw_delta: 0.15,
            aim_pitch_delta: -0.05,
            score_p1: 450.0,
            score_p2: 200.0,
        };
        let p2_pkt = PlayerInputPacket {
            frame: 42,
            player_slot: 2,
            buttons: BTN_SEND,
            movement: [-0.5, 0.5],
            aim_yaw_delta: -0.1,
            aim_pitch_delta: 0.02,
            score_p1: 450.0,
            score_p2: 200.0,
        };

        host.send_packet(&p1_pkt).unwrap();
        client.send_packet(&p2_pkt).unwrap();

        let mut rx_client = Vec::new();
        let mut rx_host = Vec::new();
        for _ in 0..50 {
            rx_client.extend(client.poll_packets());
            rx_host.extend(host.poll_packets());
            if !rx_client.is_empty() && !rx_host.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(rx_client, vec![p1_pkt]);
        assert_eq!(rx_host, vec![p2_pkt]);
        assert_eq!(host.last_remote_packet, Some(p2_pkt));
        assert_eq!(client.last_remote_packet, Some(p1_pkt));
    }
}
