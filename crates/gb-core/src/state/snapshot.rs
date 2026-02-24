use crate::apu::Apu;
use crate::bus::Bus;
use crate::config::GbConfig;
use crate::cpu::Cpu;
use crate::dma::OamDma;
use crate::error::StateError;
use crate::gameboy::GameBoy;
use crate::interrupts::InterruptController;
use crate::ppu::Ppu;
use crate::scheduler::Clock;
use crate::state::version::{SNAPSHOT_HEADER_LEN, SNAPSHOT_MAGIC, SNAPSHOT_VERSION_V1};
use crate::timers::Timers;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub flags: u16,
    pub payload_len: u32,
    pub checksum: u32,
}

#[derive(Serialize)]
struct SnapshotPayloadRef<'a> {
    cpu: &'a Cpu,
    bus: &'a Bus,
    clock: &'a Clock,
    timers: &'a Timers,
    ppu: &'a Ppu,
    apu: &'a Apu,
    interrupts: &'a InterruptController,
    dma: &'a OamDma,
    config: &'a GbConfig,
}

#[derive(Deserialize)]
pub struct SnapshotPayload {
    pub cpu: Cpu,
    pub bus: Bus,
    pub clock: Clock,
    pub timers: Timers,
    pub ppu: Ppu,
    pub apu: Apu,
    pub interrupts: InterruptController,
    pub dma: OamDma,
    pub config: GbConfig,
}

impl SnapshotHeader {
    pub fn from_payload(payload: &[u8]) -> Result<Self, StateError> {
        let payload_len =
            u32::try_from(payload.len()).map_err(|_| StateError::PayloadTooLarge {
                actual: u64::try_from(payload.len()).unwrap_or(u64::MAX),
            })?;
        Ok(Self {
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION_V1,
            flags: 0,
            payload_len,
            checksum: checksum32(payload),
        })
    }

    fn to_bytes(self) -> [u8; SNAPSHOT_HEADER_LEN] {
        let mut bytes = [0u8; SNAPSHOT_HEADER_LEN];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..6].copy_from_slice(&self.version.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.flags.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.checksum.to_le_bytes());
        bytes
    }

    fn parse(bytes: &[u8]) -> Result<Self, StateError> {
        if bytes.len() < SNAPSHOT_HEADER_LEN {
            return Err(StateError::SnapshotTooSmall {
                expected: SNAPSHOT_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        let payload_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let checksum = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        Ok(Self {
            magic,
            version,
            flags,
            payload_len,
            checksum,
        })
    }
}

pub fn encode(gb: &GameBoy) -> Result<Vec<u8>, StateError> {
    let payload = SnapshotPayloadRef {
        cpu: &gb.cpu,
        bus: &gb.bus,
        clock: &gb.clock,
        timers: &gb.timers,
        ppu: &gb.ppu,
        apu: &gb.apu,
        interrupts: &gb.interrupts,
        dma: &gb.dma,
        config: &gb.config,
    };

    let payload_bytes =
        bincode::serialize(&payload).map_err(|err| StateError::Decode(err.to_string()))?;
    let header = SnapshotHeader::from_payload(&payload_bytes)?;

    let mut out = Vec::with_capacity(SNAPSHOT_HEADER_LEN + payload_bytes.len());
    out.extend_from_slice(&header.to_bytes());
    out.extend_from_slice(&payload_bytes);
    Ok(out)
}

pub fn decode(snapshot_bytes: &[u8]) -> Result<SnapshotPayload, StateError> {
    let header = SnapshotHeader::parse(snapshot_bytes)?;

    if header.magic != SNAPSHOT_MAGIC {
        return Err(StateError::InvalidMagic {
            actual: header.magic,
        });
    }

    if header.version != SNAPSHOT_VERSION_V1 {
        return Err(StateError::UnsupportedVersion(header.version));
    }

    let actual_payload = snapshot_bytes.len().saturating_sub(SNAPSHOT_HEADER_LEN);
    let expected_payload =
        usize::try_from(header.payload_len).map_err(|_| StateError::PayloadTooLarge {
            actual: u64::from(header.payload_len),
        })?;
    if actual_payload != expected_payload {
        return Err(StateError::PayloadLengthMismatch {
            expected: expected_payload,
            actual: actual_payload,
        });
    }

    let payload = &snapshot_bytes[SNAPSHOT_HEADER_LEN..];
    let checksum = checksum32(payload);
    if checksum != header.checksum {
        return Err(StateError::ChecksumMismatch {
            expected: header.checksum,
            actual: checksum,
        });
    }

    bincode::deserialize::<SnapshotPayload>(payload)
        .map_err(|err| StateError::Decode(err.to_string()))
}

#[must_use]
pub fn checksum32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811C_9DC5u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}
